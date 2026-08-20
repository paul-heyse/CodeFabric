---
name: fastmcp-pydantic-ref
description: "Reference navigator for the two version-pinned Python deep-dives behind *serving an MCP server* and *executing a data contract*. SKILL.md maps them at `docs/library_ref/`: `fastmcp_python_advanced_reference_3.4.7.md` (server construction, tools/resources/prompts, `Context`, dependency injection, lifespans and session state, middleware, providers and transforms, auth, transports and deployment, the programmatic client, apps, CLI, testing and fingerprinting; §0-§37) and `pydantic_python_advanced_reference_2.13.4.md` (models, fields, `ConfigDict`, strict vs lax coercion, aliases, validators, serializers, `TypeAdapter`, unions, custom types, JSON Schema, errors, `pydantic-settings`, performance; §0-§51). REFERENCE.md (same folder) holds the chapter and appendix maps with line numbers, a **symbol-to-location index** for ~230 public API names, a task index, decision trees, the FastMCP-Pydantic seam, and the navigation hazards. Use when Python touches `from fastmcp import`/`FastMCP(`/`@mcp.tool`/`@mcp.resource`/`@mcp.prompt`/`Context`/`ToolResult`/`Depends`/`Provider`/`Transform`/`Middleware`/`Client(`/`fastmcp.json`/`fastmcp run`/`fastmcp inspect`, or `from pydantic import`/`BaseModel`/`ConfigDict`/`Field(`/`TypeAdapter`/`model_validate`/`model_dump`/`field_validator`/`model_validator`/`computed_field`/`RootModel`/`SerializeAsAny`/`ValidationError`/`BaseSettings`/`SettingsConfigDict`/`SecretStr`, or when pinning those packages. Rust-side parsing, storage, or query → siblings `code-facts-lib-ref`, `deltalake-rust-ref`, `datafusion-pyarrow-rust-ref`, `gix-notify-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# FastMCP + Pydantic Reference Navigator

Routes the two deep-dive references behind the **Python boundary layer** — the code that publishes an MCP surface and the code that turns untrusted input into typed values and typed values back into wire output. This SKILL.md is the **core map**: version anchors, the two-document table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the chapter and appendix indexes with line numbers, the **symbol → location index** — the most useful thing here, because both documents mention their own public API names hundreds of times and neither says which occurrence is the definition — the task index, ten decision trees, the FastMCP↔Pydantic seam, and the fourteen navigation rules. Reach for REFERENCE.md once you know which document you need; cross-references back here are written `SKILL §...`.

**These are pure library navigators.** They index what the two references say about FastMCP and Pydantic, nothing more. No project doctrine, no design-spec anchoring, no policy about which capabilities are permitted here — that belongs to whatever consumes this skill, not to the skill.

**Four ways in, and they are not interchangeable:**

| You arrive holding | Go to | Why |
|---|---|---|
| a **symbol** (`OAuthProxy`, `exclude_unset`, `TaskConfig`) | REFERENCE **§2** | grep is actively misleading here — see Rule 1 |
| a **goal** ("stop a subclass leaking extra fields") | REFERENCE **§3** | phrased the way you would phrase it, not the way the doc titles it |
| a **choice** ("`TypeAdapter` or `RootModel`?") | REFERENCE **§4** | ten decision trees |
| a **chapter number** from a citation | REFERENCE **§1** | line numbers neither document provides |

**Out of scope** (covered elsewhere): the MCP wire protocol itself — both docs describe their libraries' behavior, not the specification. FastAPI → `docs/library_ref/fastapi_python_advanced_reference_0.141.1.md` (11,004 lines), which no skill currently routes; `fastmcp` §26 covers only the FastMCP↔FastAPI *seam*. Choosing *between* Pydantic and another modelling library → sibling **`attrs-cattrs-ref`**, whose chapter 19 is the attrs/dataclasses/pydantic/msgspec comparison. Rust-side work → siblings **`code-facts-lib-ref`** (tree-sitter · Ruff · Pyrefly · rustc/MIR), **`gix-notify-ref`** (watching and Git state), **`deltalake-rust-ref`** and **`datafusion-pyarrow-rust-ref`** (storage and query).

---

## Version anchors

* **FastMCP 3.4.7** — the reference targets stable v3 throughout. Two v3 architecture facts govern almost every page: **providers and transforms became first-class primitives in 3.0.0**, and **`FastMCP()` stopped owning transport/host/port** — the constructor is identity + composition + behavior + handlers/storage, and serving is a separate later step (`run()`, `http_app()`, the CLI). Code written against v2 will construct successfully and behave wrongly; `fastmcp` §35 is the migration chapter and **§35.21 is a ready-made grep list**. Packaging splits three ways: full `fastmcp`, `fastmcp-slim[client]` for client-only footprints (§23.1), and `fastmcp-remote` as a host bridge (§23.4). **FastMCP 4 is prerelease and quarantined in §36** — do not mix its APIs into 3.x code; §36 is explicitly labelled migration research, not stable guidance.
* **Pydantic 2.13.4** (released 2026-05-06, supports Python ≥3.9) with **`pydantic-core` 2.46.4**, which Pydantic selects for itself — **never pin `pydantic-core` independently** (§1.3). Optional extras are only `[email]` and `[timezone]` (§1.1).
* **`pydantic-settings` 2.15.0 is a separately versioned package, not an extra** (§38.0) — released 2026-08-07, Python ≥3.10. It has its own extras (§51.39) and its own source-priority model (§38.2, §39.1).
* **Pydantic 2.14.0b1 is prerelease and quarantined in §47** — it drops Python 3.9, adds initial 3.15 support, and changes model-build and core-schema behavior. Treat any 2.14 example as a migration event, not a drop-in.
* **2.13-specific behavior an agent will otherwise get wrong** (§46, and the delta table at line 133): `polymorphic_serialization` is new and is the *narrow* alternative to broad `serialize_as_any` (§9.11, §19.3) · `exclude_if` now applies to computed fields (§20.2) · `StringConstraints(ascii_only=…)` is new (§11.2) · private-attribute default factories can receive validated model data (§6.5, §8.8) · discriminator-selected union branches no longer fall back across all members (§26.8) · extras assigned *after* init are now tracked in `model_fields_set` (§6.2) · and **2.13.1/2 restored `ValidationInfo.data`/`field_name` on the `model_validate_json` path** (§5.4, §33.7) — which is why 2.13.4 rather than 2.13.0.

---

## The two reference documents

Both live at `docs/library_ref/`. Each opens with a version anchor, a **"Proposed comprehensive documentation map"**, a release-delta table and a source-shorthand key, then deep-dives; each closes with a **dense appendix chapter** that is the intended fast-lookup layer. Unlike the `gix`/`notify` pair, **neither document has an end-of-reference marker** — a chapter runs until the next `# … Advanced — N)` heading, and the last chapter runs to EOF.

| Doc | Path (`docs/library_ref/`) | Lines | Chapters | Deep-dive prefix | Subsection depth |
|-----|------|------:|---|------------------|---|
| **fastmcp** | `fastmcp_python_advanced_reference_3.4.7.md` | 15,682 | **§0-§37** | `# FastMCP Advanced — N) ` | **inconsistent** — 23 chapters number at `##`, **14 number at `###`**, 13 more demote only `N.0` to `###`, §17 hides its real content one level deeper, §37 uses letters. Rule 2. |
| **pydantic** | `pydantic_python_advanced_reference_2.13.4.md` | 7,340 | **§0-§51** | `# Pydantic Advanced — N) ` | **uniform** `## N.M` throughout (594 `##` against 48 `###`). Whatever `--view expanded` shows is the whole chapter. |

**fastmcp §0-§37** — mental model and the three surfaces (servers/clients/apps) · install and packaging · a full worked first server+client+test · **the object-model map (§3: `FastMCP`, `Provider`, `Transform`, `FastMCPApp`, `Client`, composition)** · server construction and lifecycle · **tools (§5 registration, §6 typing/validation/outputs)** · resources and templates · prompts · **`Context` (§9)** · **dependency injection (§10)** · lifespans, session state and state ownership · background tasks · **middleware (§13)** · **providers (§14)** · transforms, visibility, versioning, pagination · Tool Search, Code Mode, composition, proxying, gateways · **auth (§17)** and identity-aware security policy · running and deploying · HTTP hardening and scaling · **the programmatic client (§21-§22)** · client-only packaging · apps and interactive UI · Prefab and Generative UI · OpenAPI/FastAPI integration · **`fastmcp.json` and the CLI (§27-§28)** · observability · **testing and tool fingerprinting (§30)** · host integrations · security governance · performance and large catalogs · twelve production patterns · v2→v3 migration · the v4 prerelease guide · **35 lookup matrices (§37)**.

**pydantic §0-§51** — mental model and eight core invariants · install and pinning · a worked first contract · **the architecture (§3: annotations → `CoreSchema` → Rust validator/serializer)** · `BaseModel` · **validation entry points (§5)** · trusted construction, copying, extras, field-set tracking, private state · **fields and `Annotated` (§7)** · defaults and factories · **`ConfigDict` (§9)** · **strict vs lax and the conversion contract (§10)** · reusable constrained types · **aliases (§12)** · field validators · model validators · functional validator metadata · **serialization (§16-§18)** · **polymorphic serialization and `SerializeAsAny` (§19)** · computed fields and lifecycle hooks · **`TypeAdapter` (§21)** · `RootModel` · dataclasses · `TypedDict`/`NamedTuple` · generics · **unions and discriminators (§26)** · forward refs and `model_rebuild` · dynamic models · **custom types and `CoreSchema` hooks (§29)** · standard, Pydantic-specific and network types · JSON parsing and partial validation · **JSON Schema (§34-§35)** · **errors (§36)** · `@validate_call` · **`pydantic-settings` (§38-§39)** · performance · experimental APIs · static typing · observability · framework boundaries · V1 migration · the 2.12→2.13.4 delta · the 2.14 boundary · testing · security · ten production patterns · **60 appendix subsections (§51)**.

**Reading strategy.** Start with `lib-outline <file>`, then `Read(offset, limit)` from REFERENCE.md §1. **The two docs read oppositely and want opposite tactics.**

`pydantic` chapters are **small** — median **~112** lines, range 73-234 once §51 is set aside — so read the whole chapter; it is usually cheaper than locating the right subsection. The exception dominates the document: **§51 is 1,264 lines, 17% of the file**, and it is the signature/matrix/cookbook layer. For "what is the exact 2.13.4 surface of `model_dump()`?" or "which exclusion flag?", **go to §51 first** (REFERENCE §1.4 maps its 60 subsections into four bands) and only fall back to the prose chapter for *why*.

`fastmcp` chapters are **large** — median **~400** lines, range 194-747 — so never read one whole; land on a subsection. But `lib-outline --view expanded` only helps for some of them. It is complete for **9** (§1, §2, §11, §15, §16, §18, §20, §23, and §37's lettered matrices), complete-but-for-`N.0` for **13**, and returns **nothing at all for 14** (§0, §5-§10, §12, §13, §14, §19, §21, §24, §29 — 4,575 lines, 29% of the document). Worst are §17, §22 and §27, where it returns a *partial* list that looks complete. For those, grep `^### N\.` instead. REFERENCE §1.1 carries the depth per chapter so you know before you look; Rule 2 explains it. Its appendix chapter **§37** is 747 lines of 35 lettered matrices — `J)` is the doc's own fast-lookup table and `AJ)` its source-of-truth hierarchy.

---

## Where do I look?

| Symptom / question | Go to |
|---|---|
| "Where is *X* actually documented?" | REFERENCE **§2** — never grep; `Context` has 201 hits, `TypeAdapter` 103 |
| A tool argument is being coerced when it should not be | **fastmcp** §6.3 (`strict_input_validation`) → **pydantic** §10 (where strictness is set) |
| The generated tool input schema is wrong | **fastmcp** §6.1-§6.2, §6.4 → **pydantic** §7 (`Field`/`Annotated`), §34 (JSON Schema) |
| Structured output / `ToolResult` / content blocks | **fastmcp** §6.8-§6.13, App. **H)**, App. **R)** |
| A runtime value must not appear in the MCP schema | **fastmcp** §10 (DI), §6.5, App. **B)** |
| Long-running work, progress, cancellation | **fastmcp** §12 (tasks), §9.5 (progress), App. **C)** |
| Combining or proxying servers | **fastmcp** §14, §16, §3.6, App. **D)**/**V)** |
| Which auth provider | **fastmcp** §17.1.1-§17.1.6 (a hidden `###` layer), App. **X)** |
| Where a value should live: lifespan, session, DI, `Context` | **fastmcp** §11.10 (ownership table), §11.13, §9.16 |
| Unknown input keys should fail / be kept / be dropped | **pydantic** §6.3, §9.2, §51.18 |
| Wire names differ from Python names | **pydantic** §12, §51.20 |
| A subclass is leaking fields into output | **pydantic** §19, §9.11, §51.26 |
| `None` vs missing in a PATCH body | **pydantic** §6.2, §6.8, §18.3, §8.6 |
| Validating a bare `list[int]` / union / `TypedDict` | **pydantic** §21, §51.27 |
| Reading config from env, `.env`, or a secret manager | **pydantic** §38-§39, §51.38, §51.40 |
| Validation or schema build is slow | **pydantic** §40, §51.42-§51.43 · **fastmcp** §33 |
| Verifying the published contract has not drifted | **fastmcp** §30.5-§30.9, `fastmcp inspect` (§27.11, §28.4) · **pydantic** §48.5 |

---

## Key invariants

Taken from the documents themselves — `pydantic` §0.6 and §51.60, `fastmcp` §0 and App. **AJ)**.

1. **Successful Pydantic validation describes the output, not the input.** Lax coercion is the default and is a feature; `M(x='123').x == 123`. Strictness is opt-in and settable at four levels (`pydantic` §0.6 inv. 1, §10).
2. **Validation and serialization are separate contracts.** Different aliases, different JSON Schemas (`mode='validation'` vs `'serialization'`), different behavior. Never assume one describes the other (`pydantic` §0.4, §34.2).
3. **Schema build is compile work; validation is the hot path.** Models and `TypeAdapter`s compile a Rust validator/serializer once. Constructing a `TypeAdapter` inside a loop or per request is the classic Pydantic performance bug (`pydantic` §0.6 inv. 5, §21.6, §40.2).
4. **Optionality and default are different.** `T | None` is *required and nullable* in V2. A default must be written (`pydantic` §0.6 inv. 4, §7.6).
5. **`model_construct()` skips validation and is a trust-boundary decision**, never a performance shortcut for external data (`pydantic` §0.6 inv. 3, §6.0).
6. **The FastMCP server object is identity + composition, not transport.** In v3 the constructor does not own host/port/transport; `run()`/`http_app()`/the CLI bind it later (`fastmcp` §3.0, §4.0, §35.3).
7. **Publication is not authorization.** Visibility filtering, tag filtering, Tool Search and tool annotations shape what a client *sees*; none of them is a security boundary (`fastmcp` §15.22, §16.3, §18.7, §32.4, §32.14).
8. **Pin exactly, and reconcile every example against the pin.** Both documents say the installed package's own signatures outrank the reference; both quarantine their prerelease line in a dedicated chapter (`fastmcp` App. **AJ)**, §36 · `pydantic` §0.0, §47).
