# FastMCP + Pydantic — Reference Companion

Companion to `SKILL.md` in this folder. SKILL.md is the map you read first; this file is what you
come back to once you know which document you need. Both target `docs/library_ref/`:

* **`fastmcp`** = `fastmcp_python_advanced_reference_4.0.0.md` — 12,692 lines, §0-§44
* **`pydantic`** = `pydantic_python_advanced_reference_2.13.4.md` — 7,340 lines, §0-§51

The same directory still holds `fastmcp_python_advanced_reference_3.4.7.md` (15,682 lines, §0-§37),
the **superseded v3** reference. Nothing in this file points at it; its chapter numbers overlap and
its content disagrees. Open it only to answer "what did v3 do?", and say so when you cite it.

Citations are `fastmcp §N.M` / `pydantic §N.M`, matching the documents' own numbering. Line numbers
appear only in §1, because line numbers move when a document is regenerated and section numbers do
not — seek by line, cite by section, and if a line looks wrong re-derive §1 with `lib-outline`.

| Section | What it is | Reach for it when |
|---|---|---|
| **§1** | Chapter and appendix maps, with line numbers and subsection depth | you have a section number, or you need to know where to `Read` |
| **§2** | **Symbol → canonical location**, ~240 public API names | you have a name and need the definition |
| **§3** | Task → location, phrased as goals | you have a goal and no name |
| **§4** | Eleven decision trees | you are choosing between library options |
| **§5** | The `fastmcp` ↔ `pydantic` seam | the answer might be in either document |
| **§6** | Fifteen navigation rules | before searching either file |

---

## §1 — Document maps

### §1.1 `fastmcp` — 45 chapters

Front matter: title (1) · version/source anchors and the **protocol-era rule** (5) · **"What
materially changed from the prior 3.4.7 reference"** (30) · the v4 mental-model diagram (49) ·
**"Comprehensive documentation map"** (86) · "Source index used throughout this reference" (136).
Deep-dive chapters start at 154. There is **no end-of-reference marker**; §44 runs to EOF (12,692).
Eleven chapters close with a `### Sources` block, the rest with footnote link definitions.

Two columns are load-bearing. **Depth** says whether `lib-outline --view expanded` will show you
that chapter's subsections: `**###**` means it shows *nothing*; `##` + `###`×k means it shows all
but k of them (almost always just `N.0`). See Rule 2. **Banner** marks the 28 chapters that retain
the 3.4.7 topology and open with a bold **"FastMCP 4 status."** paragraph stating that chapter's v4
delta — it sits *above* `N.0`, so read from the chapter heading, not from the first subsection
(Rule 3). Unbannered chapters were rewritten for v4 or are new to it.

| § | Line | Lines | Depth | Banner | Subs | Title |
|---|---:|---:|---|:-:|---|---|
| **§0** | 154 | 216 | **`###`** | ● | 0.1-0.8 | Scope, versioning, and mental model |
| **§1** | 370 | 204 | `##` | | 1.0-1.12 | Installation, package selection, dependency policy, and project layout |
| **§2** | 574 | 636 | `##` | ● | 2.0-2.22 | First executable server, client, and test |
| **§3** | 1210 | 97 | `##` | | 3.0-3.10 | Core API map and object model |
| **§4** | 1307 | 162 | `##` | | 4.0-4.13 | Server construction and lifecycle |
| **§5** | 1469 | 293 | **`###`** | ● | 5.0-5.13 | Tools: definition, registration, and execution contract |
| **§6** | 1762 | 329 | **`###`** | ● | 6.0-6.14 | Tools: typing, validation, hidden parameters, outputs, and content blocks |
| **§7** | 2091 | 388 | **`###`** | ● | 7.0-7.17 | Resources and resource templates |
| **§8** | 2479 | 306 | **`###`** | ● | 8.0-8.13 | Prompts and prompt rendering |
| **§9** | 2785 | 98 | `##` | | 9.0-9.8 | MCP Context |
| **§10** | 2883 | 56 | `##` | | 10.0-10.6 | Dependency injection |
| **§11** | 2939 | 91 | `##` | | 11.0-11.8 | Lifespans, request state, session state, storage, and state ownership |
| **§12** | 3030 | 87 | `##` | | 12.0-12.8 | Background tasks and long-running workflows |
| **§13** | 3117 | 446 | **`###`** | ● | 13.0-13.25 | Middleware and the server policy layer |
| **§14** | 3563 | 395 | **`###`** | ● | 14.0-14.24 | Providers and dynamic component sources |
| **§15** | 3958 | 502 | `##` | ● | 15.0-15.26 | Transforms, visibility, versioning, pagination, and discovery shaping |
| **§16** | 4460 | 419 | `##` | ● | 16.0-16.24 | Search transforms, Code Mode, composition, proxying, and gateways |
| **§17** | 4879 | 419 | `##` + `###`×18 | ● | 17.0-17.2 | Authentication and authorization |
| **§18** | 5298 | 484 | `##` | ● | 18.0-18.24 | Advanced security policy and identity-aware execution |
| **§19** | 5782 | 382 | **`###`** | ● | 19.0-19.11 | Running and deploying servers |
| **§20** | 6164 | 497 | `##` | ● | 20.0-20.26 | HTTP hardening, reverse proxies, scaling, and event delivery |
| **§21** | 6661 | 348 | **`###`** | ● | 21.0-21.12 | Programmatic client fundamentals |
| **§22** | 7009 | 693 | `##` + `###`×11 | ● | 22.0-22.18 | Client transports, handlers, roots, and client-side auth |
| **§23** | 7702 | 330 | `##` | ● | 23.0-23.19 | Client-only packaging and `fastmcp-remote` |
| **§24** | 8032 | 340 | **`###`** | ● | 24.0-24.17 | Apps and interactive UI delivery |
| **§25** | 8372 | 393 | `##` + `###`×1 | ● | 25.0-25.13 | Prefab, built-in app providers, Generative UI, and custom renderers |
| **§26** | 8765 | 425 | `##` + `###`×1 | ● | 26.0-26.15 | OpenAPI and FastAPI integration |
| **§27** | 9190 | 364 | `##` + `###`×14 | ● | 27.0-27.14 | Project configuration, settings, and portable deployment contracts |
| **§28** | 9554 | 313 | `##` + `###`×1 | ● | 28.0-28.15 | CLI and developer workflows |
| **§29** | 9867 | 197 | **`###`** | ● | 29.0-29.11 | Observability, inspection, telemetry, and operational diagnostics |
| **§30** | 10064 | 455 | `##` + `###`×1 | ● | 30.0-30.20 | Testing, contract verification, and tool fingerprinting |
| **§31** | 10519 | 304 | `##` + `###`×1 | ● | 31.0-31.17 | Ecosystem and host integrations |
| **§32** | 10823 | 466 | `##` + `###`×1 | ● | 32.0-32.24 | Security hardening and governance |
| **§33** | 11289 | 467 | `##` + `###`×1 | ● | 33.0-33.23 | Performance, scaling, resilience, and large-catalog engineering |
| **§34** | 11756 | 448 | `##` + `###`×1 | ● | 34.0-34.18 | Production architecture patterns |
| **§35** | 12204 | 98 | `##` | | 35.0-35.12 | API stability, upgrade discipline, and FastMCP 3 → 4 migration |
| **§36** | 12302 | 49 | `##` | | 36.0-36.3 | FastMCP 4 capability delta and protocol-era matrix |
| **§37** | 12351 | 114 | `##` letters | | A)-I) | Dense appendices and lookup matrices |
| **§38** | 12465 | 24 | `##` | | 38.0-38.4 | Modern multi-round interaction and request-state guards |
| **§39** | 12489 | 37 | `##` | | 39.0-39.5 | Server extensions and negotiated capabilities |
| **§40** | 12526 | 24 | `##` | | 40.0-40.4 | FastMCP 4 session state: `UserSession`, `SessionId`, `SessionProvider` |
| **§41** | 12550 | 28 | `##` | | 41.0-41.3 | Argument completion |
| **§42** | 12578 | 26 | `##` | | 42.0-42.4 | Modern identity, roles, scope challenges, and machine-to-machine authentication |
| **§43** | 12604 | 35 | `##` | | 43.0-43.4 | Client groups and multi-server orchestration |
| **§44** | 12639 | 54 | `##` | | 44.0 | FastMCP 4 production readiness and migration gate |

**§38-§44 are 228 lines for all seven.** They are the only chapters short enough to read whole, and
they carry the v4 capabilities that the retained chapters only reference. Read the whole run once
before writing v4 code.

Three chapters where the depth column understates the problem — each hides its real content one
level below what `--view expanded` reports:

* **§17** — only `17.0`, `17.1`, `17.2` are numbered at that level. Every symbol lives at
  `### 17.1.1`-`### 17.2.9`: `TokenVerifier` (4897) · `RemoteAuthProvider` (4918) ·
  `OAuthProxy` (4948) · `OIDCProxy` (4984) · `OAuthProvider` (5005) · `MultiAuth` (5032) ·
  decision framework (5070) · deployment advisories (5074) · `require_scopes(...)` (5088) ·
  `restrict_tag(...)` (5109) · AND-composition (5136) · custom/async checks (5157) ·
  component-level authz (5181) · `AuthMiddleware` (5206) · component + middleware auth (5227) ·
  access-token-aware tools (5236) · authorization advisory (5277).
* **§27** — `27.0`-`27.7` are `###` (the entire `fastmcp.json` schema: `source` 9235, `environment`
  9253, `deployment` 9280, JSON-schema support 9308, CLI override precedence 9324, auto-detection
  9338); only `27.8`-`27.13`, the CLI commands, are visible at `##`, and those carry a third level
  of their own: `27.8.1` (9376), `27.10.1`-`27.10.2` (9424/9444), `27.12.1`-`27.12.3` (9495-9515).
* **§22** — `22.0` and `22.1` are `###`; `22.2`-`22.18` are `##`. Third level:
  `22.2.1`-`22.2.3` (StdioTransport environment/path/session rules, 7054-7089), `22.3.1` (TLS,
  7133), `22.8.1` (7292), `22.9.1`-`22.9.3` (OAuth parameters/flow/guidance, 7327-7372),
  `22.10.1` (CIMD document requirements, 7399).

Six chapters also carry `####` sub-blocks under their `###` subsections, invisible to every outline
view: §0 (the five end-to-end flow stages, 210-286), §5 (the ten per-argument blocks of `5.7`,
1655-1695), §7 (the five direct resource classes in `7.9` and the three RFC 6570 forms in `7.13`,
2251-2405), §13 (the three custom-middleware patterns of `13.20`, 3424-3454), §19 (transports in
`19.2` and the two deployment shapes of `19.3`, 5838-5892), §21 (the five transports of `21.3`,
6746-6792).

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

### §1.3 `fastmcp` §37 — 9 lettered lookup matrices (12351-12464)

**This chapter shrank hard in v4**: 3.4.7 carried 35 letters over 747 lines in two overlapping
series; 4.0.0 carries **9 letters over 114 lines** in one series, `A)`-`I)`, no gaps. Everything the
old `L)`-`AJ)` run covered — transport, tool authoring, resources, prompts, the `Context` capability
matrix, visibility, auth, apps, OpenAPI, CLI, contract testing, the security and performance
checklists, the architecture chooser, the source-of-truth hierarchy — **is gone from the appendix**
and now lives only in its topical chapter, or in the per-chapter `Agent checklist` / `Anti-pattern
inventory` blocks that most chapters close with. Do not expect §37 to answer a v3-era appendix
question; it is now a v4-delta lookup layer, not a whole-document one.

| Letter | Line | Answers |
|---|---:|---|
| **A)** | 12353 | FastMCP 4 package matrix — `fastmcp` vs `fastmcp-slim[client]` vs the `[tasks]`/`[apps]`/`[code-mode]` extras vs `fastmcp-remote` |
| **B)** | 12364 | **Protocol model naming** — the eight camelCase→snake_case field renames (`inputSchema`→`input_schema`, `isError`→`is_error`, …) |
| **C)** | 12377 | **State decision table** — request state vs `request_state` vs `UserSession` vs `SessionId` vs domain store vs task backend |
| **D)** | 12388 | Interactivity decision table — modern guard vs handshake-era `ctx.elicit`, per capability |
| **E)** | 12396 | **Task truth table** — the seven rules that make `task=` work, and what the client calls |
| **F)** | 12409 | Extension vs middleware vs provider vs transform — the four-way abstraction choice |
| **G)** | 12418 | Client aggregation choices — `Client` vs `create_proxy` vs `mount` vs `ClientGroup` vs `fastmcp-remote` |
| **H)** | 12428 | **Upgrade anti-patterns** — the thirteen v3 habits that are now wrong; the closest thing to a v4 grep sheet |
| **I)** | 12444 | Production readiness checklist — condensed; §44 is the long form |

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
`Context` appears **119** times in `fastmcp`, `TypeAdapter` **103** times in `pydantic`,
`ToolResult` 32, `UserSession` 25, `Depends` 22, `ClientGroup` 17, `InputRequiredResult` 15,
`model_validate_json` 22. Every row below points at the subsection that *defines* the symbol; the
**Also** column lists the other places worth reading. **Bold** rows are new in FastMCP 4.

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

**Server object, lifecycle, serving**

| Symbol | Defined at | Also |
|---|---|---|
| `FastMCP(...)` | §3.1, §4.0-§4.2 | §3.0 the v4 ownership map · §4.1 set name/version explicitly · §4.13 the construction checklist |
| identity: `name` · `version` · `instructions` · `icons` | §4.1 | §3.1 |
| composition: `providers` · `transforms` · `middleware` · `lifespan` · `auth` | §4.2 | §3.1 the five server families · §14.20 conflict precedence |
| `lifespan=` / the `@lifespan` decorator | §4.4, §11.1 | §11.0 the state taxonomy · §20.3 ASGI forwarding · §19.6-§19.7 mounting |
| `session_state_store` | §4.5, §11.6 | §33.13 store latency · §32.17 security |
| `RequestStateSecurity` / request-state keys | §9.7, §38.3 | §4.6 · §44.0 the multi-replica gate |
| `add_middleware(...)` | §13.1 | §4.7 · §13.3 stack order |
| **`add_extension(...)`** | §4.8, §39.0 | §39.4 register on the *served* server, not a mounted child |
| **`@mcp.completion` / `add_completion_handler()`** | §4.9, §41.0 | §41.1 completion context · §41.3 the authorization caveat |
| `strict_input_validation` | §6.3 | §5.10 the execution contract — it is **server-level, not per-tool** |
| `mask_error_details` | §5.12 | §18.20 · §32.19 information disclosure |
| `list_page_size` | §15.16 | §33.8 pagination cost |
| `$ref` dereferencing at serve time | §6.2 | §33.17 its cost |
| `mcp.enable()` / `mcp.disable()` | §5.7 (Deprecated knobs) | replaced the per-component `enabled=` |
| `run()` | §19.1, §4.10 | §19.2 transport selection · §21.3 |
| `http_app()` / ASGI export | §19.3 | §20.1 · §20.2 path composition · §19.5-§19.8 mounting |
| `run_http_async()` | §19.4 | §19.10 ASGI-only knobs |
| `stateless_http` | §20.5 | §19.9 horizontal scaling · §33.12 |
| custom HTTP routes | §4.12 | §20.18 |
| `call_tool` · `read_resource` · `render_prompt` (server-side) | §3.1 | §8.11 definition lookup vs execution |
| `mount(...)` | §14.18 | §15.3 `namespace=` · §16.10 vs copy · §35.6 replaced `import_server` |
| `create_proxy(...)` / `ProxyProvider` | §14.19, §14.4 | §16.11-§16.13 · §16.0 proxies mirror the frontend era · §35.6 replaced `as_proxy` |

> **Not in the v4 reference at all:** `on_duplicate` and `client_log_level`, both documented
> constructor fields in 3.4.7, have **zero occurrences** here. Absence from this document is not
> proof of removal from the package — check the installed signatures (Rule 12).

**Protocol era and the v4 interaction model**

| Symbol | Defined at | Also |
|---|---|---|
| protocol-era negotiation · `2026-07-28` vs handshake era | §0 (the protocol-era rule), §4.11 | §3.6 · **§36.2 the era matrix** · §31.14 host negotiation strategy |
| `Client(mode="auto")` · `mode="legacy"` | §3.6, §21 banner | §9.4 · §12.3 legacy never negotiates tasks · §35.8 |
| `server/discover` | §36.2 | replaced the initialize handshake on the modern era |
| **`InputRequiredResult`** | §9.3, §38.0 | §38.4 **guards re-run from the top** · §12.6 inside a task · App. **D)** |
| `ctx.input_responses` | §9.3, §38.1 | `None` on the first leg |
| `ctx.request_state` (sealed) | §9.3, §38.1 | §9.7 sealing keys · §11.7 vs session state · App. **C)** |
| `input_required_max_rounds` | §38.2 | default 10, on the client driver |
| `ctx.elicit()` | §9.4 | **raises on `2026-07-28`** · §35.8 migration · §28.7 through the CLI |
| `ctx.sample()` · `ctx.sample_step()` · `ctx.list_roots()` | §9.5 (**removed**) | §35.5 · §4.3 the constructor half · App. **H)** |
| snake_case MCP model fields | §1.4, App. **B)** | §6 banner · §35.2 the `mcp_camelcase_compat` bridge · §30 disable it in CI |
| `FASTMCP_MCP_CAMELCASE_COMPAT` | §27 banner | migration shim only |
| `httpx2` | §1.6, §35.4 | §26 banner — OpenAPI/proxy/auth clients you pass in must migrate |

**Components**

| Symbol | Defined at | Also |
|---|---|---|
| `@mcp.tool` | §5.2 | §5.6 **the exact decorator/`Tool.from_function` surface** · §5.7 per-argument semantics |
| `mcp.add_tool(...)` | §5.3 | §5.4 standalone `@tool()` for methods |
| `@mcp.tool(auth=…)` · `timeout=` · `version=` · `task=` · `meta=` · `annotations=` | §5.7 (one `####` block each) | §17.2.5 component auth · §15.12 versioning · §12 tasks |
| deprecated `enabled=` · `exclude_args=` · `serializer=` | §5.7 (Deprecated knobs) | → `enable()`/`disable()`, `Depends()`, `ToolResult` |
| `ToolResult` | §5.11, §6.11 | §6.12 conversion rules · `ToolResult(meta=…)` is *runtime* metadata, `@mcp.tool(meta=…)` is definition metadata (§5.7) |
| `output_schema={...}` | §6.10 | §6.9 automatic generation from return annotations · must be an object type |
| `Image` · `Audio` · `File` | §6.13 | §6.12 content-block conversion rules |
| `@mcp.resource(...)` | §7.1 | §7.3 `add_resource` · §7.4 standalone `@resource()` · §7.9 the five direct resource classes |
| `ResourceResult` · `ResourceContent` | §7.6 | §7.5 return contract |
| resource templates / RFC 6570 | §7.12-§7.13 | §7.14 coercion · §7.15-§7.16 validation · §7 banner: **path-traversal screening now runs before handlers** |
| `@mcp.prompt` | §8.1 | §8.3 decorator arguments · §8.2 `add_prompt` and standalone `@prompt()` |
| `Message` | §8.7 | §8.6 return contract · §8.8 `PromptResult` |
| `PromptResult` | §8.8 | §8.9 static vs runtime metadata |

**`Context` and dependency injection**

| Symbol | Defined at | Also |
|---|---|---|
| `Context` | §9.0 (the v4 boundary diagram), §9.1 | §6.6 as a hidden injected parameter · §9.8 the agent checklist |
| `ctx.set_state()` / `get_state()` / `delete_state()` | §9.2, §11.2 | **request-local on the modern era** · §11.0 taxonomy |
| `ctx.debug/info/warning/error` · progress | §9.6 | §22.14-§22.15 the client-side `log_handler` / `progress_handler` |
| resource / prompt access from a tool | §9.0 | — |
| `ctx.request_context.protocol_version` | §9.1 | §9.4 branch on it to support both eras |
| `Depends(...)` | §10.2 | §6.5 hidden parameters · §10.1 the public-schema invariant |
| **`CallArgument(...)`** | §10.3 | binds a hidden dependency to a *public* tool argument · §10.5 not an authorization substitute |
| `CurrentContext()` · `CurrentRequest()` · `CurrentFastMCP()` | §10.0 | §9.1 for `CurrentContext` |
| `CurrentAccessToken()` | §10.0, §17.2.8 | §18.4 injection · §10.5 identity comes from auth, never from a caller argument |
| `Progress()` | §10.4 | portable across foreground and task execution |
| `CurrentDocket()` · `CurrentWorker()` | §10.4, §12.7 | **moved to `fastmcp_tasks.dependencies`** (§35.6) |

**State and background tasks**

| Symbol | Defined at | Also |
|---|---|---|
| the state taxonomy | §11.0 | §3.8 · App. **C)** · §34.13 by architecture |
| **`UserSession`** | §11.3, §40.1 | injected, hidden from the schema, **requires authentication** · §40.3 |
| **`SessionId` · `SessionProvider` · `get_session()`** | §11.4, §40.2 | §11.5 isolation — an id alone is not authority · §40.3 |
| session storage / `AsyncKeyValue` / TTL | §11.6, §40.4 | shared store for multi-replica · §32.17 |
| **`TasksExtension` / `fastmcp-tasks`** | §12.0-§12.1 | a `task=True` tool without it **fails at startup** · §1.1 packaging |
| `@mcp.tool(task=True)` | §12.1 | **tools only, async only** — v3's resource/template/prompt task surface is gone (§7-§8 banners) |
| `TaskConfig` | §12.2 | import from `fastmcp.utilities.tasks` (§35.6) · modes `forbidden`/`optional`/`required` |
| `call_tool_task(...)` | §12.3 | plain `client.call_tool()` now follows a task transparently; `call_tool(..., task=True)` is **removed** |
| task backends · workers | §12.4 | §33.14 scaling · Redis/Valkey for production |
| `FASTMCP_TASKS_ENCRYPTION_KEY` | §12.5 | the snapshot holds caller credentials · §32.18 |
| interactive tasks | §12.6 | guard pattern, never a blocking `ctx.elicit()` |

**Middleware and extensions**

| Symbol | Defined at | Also |
|---|---|---|
| `Middleware` · `add_middleware(...)` | §13.1 | §13.20 subclassing · §13.2 `call_next(context)` · §13.3 stack order |
| hooks: `on_message` · `on_request` · `on_call_tool` · `on_list_tools` … | §13.5 | §13.6 signature · §13.9 operation hooks · §13.11 raw `__call__` |
| `MiddlewareContext` | §13.6 | §13.22 component metadata · §13.23 storing state |
| `on_initialize` | §13.10 | carries a hard timing rule · **§36.2: no initialize event on the modern era** |
| the widened v4 message surface | §13 banner, §3.4 | §35.10 audit counters and generic hooks |
| built-in middleware | logging §13.13 · timing §13.14 · **caching §13.15** · rate limiting §13.16 · error handling §13.17 · retry §13.18 · ping §13.19 | §32.16 caching security · §33.15 overhead |
| **`ServerExtension`** | §39.0, §3.5 | §4.8 registration · App. **F)** vs middleware/provider/transform |
| `MethodBinding` | §39.2 | additive methods only — **may not shadow `tools/call`** |
| `intercept_tool_call()` | §39.3 | runs after middleware, before the tool body; nests registration-order-outermost |
| `client_extension_settings(...)` | §39.1 | **negotiation is per request** — check opt-in before changing semantics |
| `ClientExtension` · `Client(extensions=[...])` | §39.5 | `advertise()` only when the client really implements it |

**Providers, transforms, discovery**

| Symbol | Defined at | Also |
|---|---|---|
| `Provider` | §14.1, §3.2 | §14.0 source vs rewrite vs policy layer · §14.23 provider vs middleware |
| `LocalProvider` | §14.2 | §3.1 direct registrations land here |
| `FastMCPProvider` | §14.3 | §16.9 composing focused child servers |
| `ProxyProvider` | §14.4 | §14.5 session models · §14.6 feature forwarding · §32.9 credential separation |
| `FileSystemProvider` | §14.7 | §31.12 |
| `SkillsDirectoryProvider` | §14.8 | **renamed from v3's `SkillsProvider`** (§14 banner) |
| `OpenAPIProvider` | §26.2 | §26.3 default route mapping · §32.10 SSRF · new import home `fastmcp.server.providers.openapi` (§35.6) |
| `Transform` | §14.10, §3.3 | §15.1 provider vs server level · §15.19 order is API behavior |
| `add_transform(...)` vs `wrap_transform(...)` | §14.12 | §15.20 wrap for reusable providers |
| `Namespace(...)` | §14.13 | §15.2 · §16.15 · `mount(namespace=…)` replaced `prefix=` (§35.6) |
| `ToolTransform(...)` | §14.14 | §15.4 · §15.5 transform vs wrapper tool |
| `ToolSearch(...)` | §14.15 | §16.1-§16.3 · §33.6 · **§16.3 search is not security** |
| `ResourcesAsTools` · `PromptsAsTools` | §14.16-§14.17 | §15.6-§15.7 |
| Code Mode | §16.4-§16.8 | §16.21 suitability matrix · §32.12 security · §33.7 |
| visibility / `only=True` / component keys | §15.8-§15.10 | §15.22 publication is not authorization · §18.7 · §32.4 |
| versioned components | §15.12-§15.14 | §15.15 versioning is not migration · §30.10 |

**Auth and identity** — the six provider families live under §17's hidden third level (see §1.1); the v4 additions are a separate chapter.

| Symbol | Defined at | Also |
|---|---|---|
| `TokenVerifier` | §17.1.1 | §18.8 verifier invariants |
| `RemoteAuthProvider` | §17.1.2 | — |
| `OAuthProxy` | §17.1.3 | §18.10 trust boundary · §32.7 |
| `OIDCProxy` | §17.1.4 | — |
| `OAuthProvider` | §17.1.5 | — |
| `MultiAuth` | §17.1.6 | §17.1.7 decision framework · §17.1.8 deployment advisories |
| `require_scopes(...)` | §17.2.1 | §18.6 scope vs resource checks · §5.7 as `@mcp.tool(auth=…)` |
| `restrict_tag(...)` | §17.2.2 | §17.2.3 AND-composition · §17.2.4 custom/async checks |
| `AuthMiddleware` | §17.2.6 | §17.2.7 component + middleware auth · §18.3 |
| **`require_roles(...)` / roles vs scopes** | §42.1 | §17 banner · keep roles an authorization check, not a visibility tag |
| **insufficient-scope challenges** | §42.2 | names the missing scopes instead of an opaque denial |
| **identity assertion (SEP-990, beta)** | §42.3 | validate audience/issuer/expiry before using it as a tenant boundary |
| **client-credentials auth** | §42.4 | non-interactive service→service; rotate separately from human OAuth clients |
| CIMD / client assertions | §18.11 | §22.10 client side · §32.8 |

**Client**

| Symbol | Defined at | Also |
|---|---|---|
| `Client(...)` | §21.1, §22.0-§22.1 | §21.2 `async with`, reentrancy and `auto_initialize=False` · §22.1 prefer explicit transports in production |
| `Client(Path("server.py"))` | §1.8 | **bare script strings are deprecated** (§35.11) — strings converge on URLs in FastMCP 5 |
| `ping()` | §21.5 | **not routed on the modern sessionless era** (§21 banner) |
| `list_tools()` · `list_resources()` · `list_prompts()` | §21.6 | — |
| `call_tool(...)` (client) | §21.7 | §21.10 `.data` vs `.content` · §12.3 it now follows a task transparently |
| `read_resource(...)` · `get_prompt(...)` | §21.8-§21.9 | §21.10 |
| `StdioTransport` | §22.2 | **§22.2.1 nothing is inherited from the parent environment** · §22.2.3 session reuse |
| `StreamableHttpTransport` | §22.3 | §22.3.1 TLS |
| `SSETransport` | §22.4 | backward compatibility only |
| in-memory client/server | §22.5 | §2.3 · §30.2 **the default integration-test primitive** |
| `MCPConfig` / config-dict transports | §22.6 | §21.1 inference rules |
| `auth="<token>"` / `BearerAuth(...)` | §22.8 | §22.8.1 |
| `auth="oauth"` / `OAuth(...)` | §22.9 | §22.9.1 parameters that matter · §22.9.2 flow and token storage · §22.10 CIMD |
| `message_handler` · `sampling_handler` · `elicitation_handler` · `log_handler` · `progress_handler` | §22.11-§22.15 | §22 banner: they serve **both** legacy pushed requests and modern returned input requests |
| roots | §22.16 | static and dynamic forms — the *server* method is removed (§9.5) |
| **`ClientGroup`** | §43.0-§43.1 | §43.2 `resolve_tool()` provenance · §43.4 consume vs republish · App. **G)** |
| `fastmcp-slim[client]` · `fastmcp-remote` | §23.1, §23.4 | §23.11-§23.12 vs `Client` and vs a gateway · §1.1, §1.7 · App. **A)** |

**Apps, config, CLI, testing**

| Symbol | Defined at | Also |
|---|---|---|
| `@mcp.tool(app=True)` (Prefab) | §24.2, §25.2 | §24.3 · §25.9 **pin `prefab-ui` yourself** |
| `FastMCPApp` | §24.4, §25.3, §3.9 | §24.7 why it exists · new import home `fastmcp.apps` (§35.6) |
| `@app.ui()` · `@app.tool()` | §24.5-§24.6 | §25.3 string backend names are fragile |
| `CallTool(...)` · `result_key` | §24.8 | — |
| `GenerativeUI()` | §24.10, §25.8 | §32.13 security · §25.8 sandbox is necessary, not sufficient |
| `Approval` · `Choice` · `FormInput` · `FileUpload` | §25.4-§25.7 | §25.4 **`Approval` is UX, not a security boundary** (§32.15) · §25.7 file security checklist |
| `AppConfig` · `ui://` resources | §24.12-§24.13 | §24.14 CSP and permissions · §24.15 host support detection |
| `FastMCP.from_openapi(...)` | §26.1 | §26.11 generated vs model-facing schemas · pass an `httpx2.AsyncClient` |
| `FastMCP.from_fastapi(...)` | §26.9 | §26.10 conversion vs mounting |
| `RouteMap` · `MCPType` | §26.4 | §26.5 exclusion-first policy |
| `fastmcp.json` | §27.0-§27.7 (all `###`) | `source` §27.2 · `environment` §27.3 · `deployment` §27.4 · schema §27.5 · precedence §27.6 · §27.13 vs standard MCP JSON |
| `fastmcp run` | §27.8, §28.2 | §27.8.1 dependency behavior |
| `fastmcp inspect` | §27.11, §28.4 | §29.5 the primary manifest check · §30.9 snapshots |
| `fastmcp install` | §27.10, §28.9 | §27.10.1 `mcp-json` · §27.10.2 `stdio` |
| `fastmcp generate-cli` | §27.12, §28.10 | §27.12.1-§27.12.3 · §28.10 drift warning |
| `fastmcp dev` · `list` · `call` · `discover` | §28.3, §28.5-§28.6, §28.8 | §28.12 debugging decision tree · §24.16 `fastmcp dev apps` |
| tool fingerprinting | §30.5 | §30.6 what belongs in one · §30.8 drift classification · §30.9 |
| protocol-mode acceptance tests | §30.15, §30.11 | §31.15 cross-host suite · §44.0 |
| OpenTelemetry instrumentation | §29.1-§29.3 | one SERVER span per request, with protocol version (§29 banner) · §33.20 |

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
| register a tool / resource / prompt | `fastmcp` §5.2 / §7.1 / §8.1 · §5.6 the exact decorator surface |
| decide whether something should be a tool, a resource or a prompt | §4 tree 8 · `fastmcp` §5.0, §7.0, §8.0 |
| hide a runtime-only value from the published schema | `fastmcp` §10.1-§10.2 (DI), §6.5 |
| hide a value that still has to depend on a public tool argument | `fastmcp` §10.3 `CallArgument` |
| get logging or progress from inside a handler | `fastmcp` §9.6 · §22.14-§22.15 for the client half |
| ask the caller a question mid-execution | `fastmcp` §9.3 + §38 (`InputRequiredResult`) · §9.4 for the legacy path · App. **D)** |
| open a database pool once for the process | `fastmcp` §4.4, §11.1 (lifespan) |
| decide where a value lives — lifespan, request, session, domain | `fastmcp` §11.0 taxonomy · App. **C)** · §34.13 by architecture |
| keep state between two separate tool calls | `fastmcp` §11.3 `UserSession` / §11.4 `SessionId` · §40 |
| run work that outlives one request | `fastmcp` §12.0-§12.1 (install `fastmcp-tasks`, register `TasksExtension`) · App. **E)** |
| add cross-cutting policy (logging, rate limit, retry, cache) | `fastmcp` §13.13-§13.19 built-ins · §13.20 custom |
| add a protocol capability of my own | `fastmcp` §39 `ServerExtension` · App. **F)** for the four-way choice |
| autocomplete a prompt argument or template parameter | `fastmcp` §41 · §4.9 registration · §41.3 authorization caveat |
| combine several servers into one surface | `fastmcp` §14.18 `mount` · §16.9-§16.10 |
| put a remote MCP server behind this one | `fastmcp` §14.19 `create_proxy`, §16.11-§16.13 |
| consume several servers without republishing them | `fastmcp` §43 `ClientGroup` · App. **G)** |
| stop a large catalog eating the model's context | `fastmcp` §14.15 `ToolSearch`, §15.16 pagination, §33.5-§33.6 |
| rename or reshape a tool coming from a provider | `fastmcp` §14.14 `ToolTransform` · §15.4-§15.5 |
| publish two versions of one tool name | `fastmcp` §15.12-§15.14 |
| turn an OpenAPI spec or FastAPI app into a server | `fastmcp` §26.1 / §26.9 (pass an `httpx2` client) |
| pick an auth provider | `fastmcp` §17.1.1-§17.1.7 |
| authorize a specific tool | `fastmcp` §17.2.1-§17.2.5 · §18.3 · §42.1 for roles |
| choose STDIO vs HTTP, and deploy it | `fastmcp` §19.2-§19.3, §20 |
| make the project reproducible for a host | `fastmcp` §27.0-§27.7 `fastmcp.json` · §27.10 install |
| test it | `fastmcp` §30.2 in-memory first · §2.12 pytest fixture · §30.3-§30.4 · §30.15 protocol parity |
| upgrade from v3 | `fastmcp` **§35** · App. **H)** anti-patterns · §35.12 and §44 the gates |

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
| stop raw input appearing in error output | `pydantic` §9.10, §36.7 · §49.9 · `fastmcp` §5.12 `mask_error_details` |
| work out which of two libraries owns the bug | **§5** |
| understand why a v3 FastMCP example does not work | `fastmcp` §35 (start at §35.5-§35.6) · App. **H)** · §36.2 the era matrix |
| understand why an `AttributeError` names a camelCase field | `fastmcp` §1.4, App. **B)** · §35.2 the compat bridge |
| understand why `ctx.elicit()` / `ctx.sample()` raises | `fastmcp` §9.4-§9.5, §35.5, §36.2 |
| understand why state vanished between two calls | `fastmcp` §9.2, §11.0, §35.9 |
| understand why a 2.14 Pydantic example does not work | `pydantic` §47 |

---

## §4 — Decision trees

Eleven choices the two libraries force on you. Each tree ends in a citation; the citation is the
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

**8. Tool, resource, or prompt?** (`fastmcp` §5.0, §7.0, §8.0)

```
the model should be able to *do* something, with arguments
  -> tool                                          §5
the model should be able to *read* something, addressed by URI
  -> resource, or a resource template if parameterised   §7.1, §7.12
the *user* picks a reusable message scaffold
  -> prompt                                        §8.1
the client only supports tools
  -> ResourcesAsTools / PromptsAsTools             §14.16-§14.17
... and only tools may carry task=True in v4       §12.1, App. E)
```

**9. Compose, mount, proxy, or group?** (`fastmcp` §3.10, §14.18-§14.19, §43.4, App. **G)**)

```
components declared in this process
  -> LocalProvider, implicitly                     §14.2
another FastMCP server object, live and nested
  -> mount(child, namespace=...)                   §14.18, §15.3
   ... import_server() and mount(prefix=) are gone §35.6
a remote MCP server, republished under this one
  -> create_proxy(...) / ProxyProvider             §14.19, §16.11
   ... the proxy mirrors the frontend protocol era on its backend   §16 banner
several servers you only want to *consume*, each keeping its own
auth, handlers and protocol era
  -> ClientGroup                                   §43.0-§43.1
a whole catalog from a spec, filesystem or skills directory
  -> OpenAPIProvider / FileSystemProvider / SkillsDirectoryProvider   §26.2, §14.7, §14.8
a stdio-only host that must reach a remote HTTP server
  -> fastmcp-remote                                §1.7, §23.4
you only need to rename/reshape what a source already gives you
  -> a Transform, not a new provider               §14.10, §15.5
```

**10. Where does this value come from, and how long does it live?** (`fastmcp` §11.0, App. **C)**)

```
the model supplies it
  -> an ordinary typed parameter                   §5.7
the runtime supplies it and the model must not see it
  -> DI: Depends(...) / CurrentContext() / CurrentAccessToken()   §10.2, §10.0
the hidden dependency needs a *public* argument's validated value
  -> Depends(f, arg=CallArgument("public_name"))   §10.3
per-request MCP capability (log, progress, resource read)
  -> Context                                       §9.0-§9.1
coordination between middleware and the tool, this request only
  -> ctx.set_state()/get_state()                   §9.2, §11.2   -- NOT across calls
small opaque value carried between the legs of ONE guard interaction
  -> sealed ctx.request_state                      §9.3, §38.1   -- needs a shared key ring §38.3
one bucket per authenticated user, across calls
  -> UserSession                                   §11.3, §40.1  -- requires auth
several named buckets per user, caller passes the handle
  -> SessionId + SessionProvider + get_session()   §11.4, §40.2
per-process and expensive to build (pool, client, model)
  -> lifespan                                      §4.4, §11.1
durable business truth
  -> your own domain store                         §11.7
background execution status/result
  -> the tasks backend                             §12.4
never
  -> a module global, or a modern HTTP connection object   §11.8
```

**11. Which interaction and protocol era?** (`fastmcp` §36.2, §9.3-§9.5, App. **D)**)

```
the tool needs input from the caller mid-execution
  modern 2026-07-28  -> return InputRequiredResult; re-read ctx.input_responses   §9.3, §38
  handshake era only -> await ctx.elicit(...)                                     §9.4
   ... ctx.elicit() RAISES on the modern era; branch on
       ctx.request_context.protocol_version if you must serve both     §9.4
the server wants the client's model to generate something
  -> ctx.sample()/sample_step() are REMOVED         §9.5, §35.5
     call a model provider directly from server code, or guard for it
the server wants filesystem roots
  -> ctx.list_roots() is REMOVED                    §9.5
     take paths as explicit tool arguments, or guard for them
you control every caller and cannot port a handshake-only capability yet
  -> Client(mode="legacy"), as a transitional step  §35.8, §36.3
you serve public or uncontrolled clients
  -> design for modern sessionless semantics        §36.3
you claim to support both
  -> test both; §30.15 parity tests, §44.0 the gate
```

---

## §5 — The `fastmcp` ↔ `pydantic` seam

Each document treats the other library as background, so the seam is where an agent loses the
thread. Five crossings, and one rule.

| Crossing | `fastmcp` side | `pydantic` side |
|---|---|---|
| **A tool signature is a schema.** Type hints on the function become the published input schema. | §6.1 generation from hints · §6.2 serve-time shaping and `$ref` dereferencing · §6.4 `Annotated[...]`/`Field(...)` on parameters · §2.6 worked example | §7 fields and `Annotated` · §34 JSON Schema · §29 for a type that will not schematise |
| **Input coercion policy.** Flexible mode allows Pydantic-style coercion so `"10"` satisfies `int`; strict mode validates against the exact generated schema first, and it is **server-level, not per-tool**. | §6.3 `strict_input_validation` · §5.10 the execution contract | §10 strict vs lax, and the four places to set it · §10.5 for JSON-specific behavior |
| **Output shape.** What a return value becomes on the wire — content blocks, structured content, output schema. | §6.8-§6.10 · §6.11 `ToolResult` · §6.12 conversion rules | §16 `model_dump` modes · §18 exclusion · §19 subclass leakage · §34.2 serialization-mode schema |
| **Server configuration.** FastMCP has `fastmcp.json` for the *project*; typed process settings are Pydantic's job. | §27.0-§27.7 `fastmcp.json` · §27.6 CLI override precedence | §38-§39 `pydantic-settings` · §51.38 source map |
| **The version floor.** FastMCP 4 requires **Pydantic ≥2.12**, and extension `MethodBinding` request params are validated with Pydantic models. | §1.0 dependency floors · §35.1 · §39.2 | §1 install and pinning — 2.13.4 clears the floor |

**The rule.** In a traceback that spans both, split it at the schema. Anything about *whether a
value was accepted, coerced, or rejected*, and anything about *what a dumped object looks like*, is
a `pydantic` question even when the symbol in the traceback is a FastMCP one. Anything about
*whether the component was registered, visible, routed, authorized, or negotiated* is a `fastmcp`
question even when the payload is a model.

Three consequences worth stating outright:

* **`pydantic` §19 applies to tool return values.** A tool annotated `-> Base` that returns a
  `Derived` will not emit `Derived`'s extra fields, because that is Pydantic's annotation-driven
  default (§19.0) — not a FastMCP bug.
* **`pydantic` §21.6 applies to tool handlers.** A `TypeAdapter` constructed inside a handler is
  recompiled on every call. Build it at module scope. It applies twice over to a **guard** tool,
  whose body re-executes on every interaction round (`fastmcp` §38.4).
* **The MCP model rename is not a Pydantic alias question.** `input_schema` vs `inputSchema` is the
  MCP SDK v2 field/alias split (`fastmcp` §1.4, App. **B)**), not something `serialize_by_alias` or
  `validate_by_name` controls on *your* models.

---

## §6 — Navigation rules

Fifteen rules, all verified against the two files. Rules 1-6 are about finding things; 7-10 about
reading them; 11-15 about trusting what you find.

**1. Look symbols up in §2, never by grepping.** These documents teach by example, so a public API
name appears wherever it is *used*, not only where it is defined. Measured hit counts: `Context`
**119** in `fastmcp`, `TypeAdapter` **103** in `pydantic`, `ToolResult` 32, `UserSession` 25,
`Depends` 22, `model_validate_json` 22, `ClientGroup` 17, `ToolTransform` 15,
`InputRequiredResult` 15, `SecretStr` 12. A grep gives you the noise and hides the definition
inside it.

**2. `lib-outline --view expanded` is only trustworthy on `pydantic`.** That extractor maps `##` to
members, and `fastmcp`'s subsection depth is inconsistent:

* **11 chapters number their subsections at `###`** — §0, §5, §6, §7, §8, §13, §14, §19, §21, §24,
  §29. Together **3,640 lines, 28.7% of the document.** Expanding them returns *nothing at all*,
  silently. That includes tools, middleware and providers — three of the highest-traffic chapters
  in the file.
* **§17, §22 and §27 return a partial list**, which is worse than nothing because it looks
  complete. §17 shows 2 entries and hides all six auth providers plus all nine authorization
  subsections one level below; §27 shows 7 and hides the entire `fastmcp.json` schema; §22 hides
  `22.0`-`22.1` and nine third-level blocks.
* **8 chapters** demote only `N.0` to `###`, so just their orientation subsection is missing —
  §25, §26, §28, §30, §31, §32, §33, §34.
* **23 chapters expand completely**: §1-§4, §9-§12, §15, §16, §18, §20, §23, §35, §36, §38-§44,
  and §37 (whose letters are `##`, so they list correctly even though they are not `N.M`).

Use §1.1's Depth column to know in advance. For a `###`-numbered chapter, grep `^### N\.` instead —
and remember that six chapters carry `####` blocks that no outline view reaches at all (§1.1).
`pydantic` has no equivalent problem: 594 `##` against 48 `###`, uniform `## N.M` in all 52 chapters.

**3. In `fastmcp`, read from the chapter heading, not from `N.0`.** 28 of the 45 chapters — §0, §2,
§5-§8, and every chapter §13-§34 — retain the 3.4.7 topology and state their v4 delta in a bold
**"FastMCP 4 status."** paragraph placed *above* the first numbered subsection. It is often the only
place a chapter says that something was removed, renamed or re-scoped (that `task=` left prompts,
that `SkillsProvider` became `SkillsDirectoryProvider`, that OpenAPI clients must now be `httpx2`).
A `Read` that seeks to `N.0` skips all of it. The other 17 chapters (§1, §3, §4, §9-§12, §35-§44)
were rewritten for v4 or are new, and carry no banner.

**4. A bare `rg '^# '` is roughly half wrong on both files.** It reports **81** top-level headings in
`fastmcp` and **78** in `pydantic`; the real counts are **46** and **57**. The difference is
`#`-prefixed shell and Python comments inside fenced blocks — **35 decoys** in `fastmcp`, **21** in
`pydantic` — things like `# obsolete in v4`, `# ASGI export`, `# contracts/types.py`.
`lib-outline` parses markdown and is immune; a hand-rolled search needs a fence toggle.

**5. Build indexes from chapter headings, never from either document's own map.** The
"Comprehensive documentation map" is an outline written before the chapters and never reconciled.
In the 4.0.0 `fastmcp` reference **25 of 45 titles diverge** — §9 ("MCP Context in FastMCP 4" →
"MCP Context"), §10 (drops "and call-argument binding"), §12 ("through the tasks extension" →
"and long-running workflows"), §22 (the map promises "protocol modes", the chapter says
"client-side auth"), and 21 more. `pydantic` diverges on one (§0). The chapter heading is the real
title; the map is useful only as a table of contents.

**6. Neither document has an end-of-reference marker.** `gix`/`notify` end with `# End of reference`;
these do not. A chapter runs until the next `# … Advanced — N)` heading, and the final chapter runs
to EOF: `fastmcp` §44 ends in a checklist block, `pydantic` §51 is followed only by
`# Reference source URLs` (7294). Eleven `fastmcp` chapters close with a `### Sources` block and the
rest with footnote link definitions — both look like content to a line-window read.

**7. Numbering starts in three different places.** `fastmcp` §0 has **no `0.0`** (it opens at `0.1`);
`pydantic` §51 has **no `51.0`** (it opens at `51.1`); everything else opens at `N.0`. And every one
of `fastmcp`'s `###`-numbered chapters opens with a *bare, unnumbered* `###` line restating the
chapter title, usually followed by the status banner, before the first real subsection. Seeking to
"the first subsection" lands on that echo.

**8. Read `pydantic` by the chapter, `fastmcp` by the subsection — except §38-§44.** `pydantic`
chapters have a median of **~112** lines (range 73-234 once §51 is set aside); reading the whole
chapter usually costs less than locating the right part of it. `fastmcp` §0-§37 have a median of
**~344** (up to 693) — never read one whole. But `fastmcp` §38-§44 are **24-54 lines each, 228 for
all seven**, and they carry the v4 capabilities the retained chapters only cross-reference: read
that whole run once.

**9. Blank-line density is ~30% in both** — 31.4% in `fastmcp`, 30.3% in `pydantic` — so a
`Read(offset, limit)` window holds roughly what it appears to. (The `gix` reference in this same
directory is 53%, which is why that skill warns to double the limit; these two do not need it.)

**10. The two appendix chapters are no longer comparable in weight.** `pydantic` §51 is still the
intended lookup layer — 1,264 lines, **17% of the file**, indexed at §1.4; go there first for a
signature or a matrix. `fastmcp` §37 shrank from 35 lettered matrices to **9** (114 lines, §1.3) and
now covers only the v4 delta: packages, the field rename, state, interactivity, tasks, the
abstraction choice, client aggregation, upgrade anti-patterns, readiness. Everything the old
`L)`-`AJ)` series answered — the `Context` capability matrix, transport and auth matrices, the tool
and resource quick matrices, the CLI and testing matrices, the source-of-truth hierarchy — is only
in the topical chapters now, or in the per-chapter `Agent checklist` / `Anti-pattern inventory`
blocks. Do not send a v3-era appendix question to §37.

**11. A named symbol is not necessarily in the document.** `JsonValue`, for one, is never mentioned
in the `pydantic` reference; `on_duplicate` and `client_log_level`, both documented `FastMCP(...)`
fields in 3.4.7, have zero occurrences in the 4.0.0 reference. Absence from §2 means absence from
the document, **not** absence from the library — fall back to the installed package.

**12. Both documents rank themselves below the installed package.** `pydantic` §0.0 and the
`fastmcp` source-precedence list (front matter, §1.3) give near-identical hierarchies: the project's
pinned version and its actual signatures outrank the official docs, which outrank these references.
`fastmcp` additionally ranks PyPI and the official 3→4 upgrade guide above its own prose, and §5.6
openly notes that the docs surface and the source-level `Tool.from_function(...)` surface differ.
Reconcile any example against the pin before applying it.

**13. Only `pydantic` still quarantines a prerelease.** `pydantic` §47 (74 lines) is 2.14, written
in the same voice as the stable chapters; if a symbol only resolves there, it does not exist in the
pinned version. The `fastmcp` reference no longer has such a chapter — v4 **is** the stable
baseline, §35 is the 3→4 migration and §36 is the shipped capability delta. The only forward-looking
claim in it is §35.11's note that string client sources converge on URL semantics in FastMCP 5.

**14. Version-sensitive claims live in the delta chapters, not the feature chapters.** Behavior that
changed is described *twice*: once in the topical chapter (usually only in its status banner) and
once in `fastmcp` §35-§36 / `pydantic` §46. The delta chapter is the one that says what the old
behavior was. For a "this used to work" question start there — `fastmcp` §35.5-§35.6 lists the
removals and the new import homes, App. **H)** is the closest thing to a v4 grep sheet, and
`pydantic` §51.44-§51.47 catch the three classic V1→V2 traps.

**15. Two `fastmcp` references sit in the same directory, and a glob will match both.**
`fastmcp_python_advanced_reference_4.0.0.md` is the one this file indexes;
`fastmcp_python_advanced_reference_3.4.7.md` (15,682 lines) is the superseded v3 reference. They
share chapter numbers and disagree on content — 3.4.7 §36 is a *prerelease* v4 guide whose API
sketches were not all borne out, and its §9-§12 describe the callback-channel model v4 removed.
Always name the file you read. Note that `docs/spec_index/library-routing.md` still routes FastMCP
questions to the 3.4.7 filename; that index is derived navigation, never normative, and this skill
is the more current pointer.
