---
name: fastmcp-pydantic-ref
description: "Reference navigator for the two version-pinned Python deep-dives behind *serving an MCP server* and *executing a data contract*. SKILL.md maps them at `docs/library_ref/`: `fastmcp_python_advanced_reference_4.0.0.md` (protocol-era negotiation, server construction, tools/resources/prompts, `Context`, dependency injection, request and session state, the tasks extension, middleware, providers and transforms, server extensions, completion, auth and identity, transports and deployment, the programmatic client and `ClientGroup`, apps, CLI, testing and fingerprinting, 3→4 migration; §0-§44) and `pydantic_python_advanced_reference_2.13.4.md` (models, fields, `ConfigDict`, strict vs lax coercion, aliases, validators, serializers, `TypeAdapter`, unions, custom types, JSON Schema, errors, `pydantic-settings`, performance; §0-§51). REFERENCE.md (same folder) holds the chapter and appendix maps with line numbers, a **symbol-to-location index** for ~240 public API names, a task index, decision trees, the FastMCP-Pydantic seam, and the navigation hazards. Use when Python touches `from fastmcp import`/`FastMCP(`/`@mcp.tool`/`@mcp.resource`/`@mcp.prompt`/`@mcp.completion`/`Context`/`ToolResult`/`InputRequiredResult`/`UserSession`/`SessionId`/`Depends`/`CallArgument`/`Provider`/`Transform`/`Middleware`/`ServerExtension`/`add_extension`/`TasksExtension`/`Client(`/`ClientGroup`/`fastmcp.json`/`fastmcp run`/`fastmcp inspect`, or `from pydantic import`/`BaseModel`/`ConfigDict`/`Field(`/`TypeAdapter`/`model_validate`/`model_dump`/`field_validator`/`model_validator`/`computed_field`/`RootModel`/`SerializeAsAny`/`ValidationError`/`BaseSettings`/`SettingsConfigDict`/`SecretStr`, or when pinning those packages. Rust-side parsing, storage, or query → siblings `code-facts-lib-ref`, `deltalake-rust-ref`, `datafusion-pyarrow-rust-ref`, `gix-notify-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# FastMCP + Pydantic Reference Navigator

Routes the two deep-dive references behind the **Python boundary layer** — the code that publishes an MCP surface and the code that turns untrusted input into typed values and typed values back into wire output. This SKILL.md is the **core map**: version anchors, the two-document table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the chapter and appendix indexes with line numbers, the **symbol → location index** — the most useful thing here, because both documents mention their own public API names hundreds of times and neither says which occurrence is the definition — the task index, eleven decision trees, the FastMCP↔Pydantic seam, and the fifteen navigation rules. Reach for REFERENCE.md once you know which document you need; cross-references back here are written `SKILL §...`.

**These are pure library navigators.** They index what the two references say about FastMCP and Pydantic, nothing more. No project doctrine, no design-spec anchoring, no policy about which capabilities are permitted here — that belongs to whatever consumes this skill, not to the skill.

**Four ways in, and they are not interchangeable:**

| You arrive holding | Go to | Why |
|---|---|---|
| a **symbol** (`OAuthProxy`, `UserSession`, `exclude_unset`) | REFERENCE **§2** | grep is actively misleading here — see Rule 1 |
| a **goal** ("stop a subclass leaking extra fields") | REFERENCE **§3** | phrased the way you would phrase it, not the way the doc titles it |
| a **choice** ("`TypeAdapter` or `RootModel`?", "guard or `ctx.elicit`?") | REFERENCE **§4** | eleven decision trees |
| a **chapter number** from a citation | REFERENCE **§1** | line numbers neither document provides |

**Out of scope** (covered elsewhere): the MCP wire protocol itself — both docs describe their libraries' behavior, not the specification. FastAPI → `docs/library_ref/fastapi_python_advanced_reference_0.141.1.md` (11,004 lines), which no skill currently routes; `fastmcp` §26 covers only the FastMCP↔FastAPI *seam*. Choosing *between* Pydantic and another modelling library → sibling **`attrs-cattrs-ref`**, whose chapter 19 is the attrs/dataclasses/pydantic/msgspec comparison. Rust-side work → siblings **`code-facts-lib-ref`** (tree-sitter · Ruff · Pyrefly · rustc/MIR), **`gix-notify-ref`** (watching and Git state), **`deltalake-rust-ref`** and **`datafusion-pyarrow-rust-ref`** (storage and query).

---

## Version anchors

* **FastMCP 4.0.0 GA**, released 2026-08-31 — the reference targets stable v4 throughout, on the MCP Python SDK v2. **The one fact that governs almost every page is protocol-era negotiation**: a single v4 server can serve the modern **`2026-07-28`** era, which is **sessionless and self-contained**, *and* older handshake-era clients, and the era is negotiated **per connection**, not chosen process-wide. `Client(mode="auto")` is the default; `mode="legacy"` pins the handshake era (`fastmcp` §0, §3.6, §4.11, §36.2). Everything that used to depend on a live server→client callback channel changed as a result — see the removals below.
* **Environment floors that break an upgrade before any code does** (§1.0, §35.1): Python **≥3.10** · Pydantic **≥2.12** · Starlette **≥1.0.1** for the server extra · FastAPI **≥0.133.0** when FastAPI owns the ASGI app · and FastMCP's own HTTP stack is **`httpx2`, not `httpx`** — client factories, auth objects, OpenAPI clients and exception handlers you pass *into* FastMCP must migrate (§1.6, §35.4).
* **Removed in v4 — code using these constructs successfully in v3 will now fail** (§35.5-§35.6, App. **H)**): `ctx.sample()` · `ctx.sample_step()` · `ctx.list_roots()` · `FastMCP(sampling_handler=…)` · `sampling_handler_behavior` · `client.call_tool(..., task=True)` · `task=` on resources, templates and prompts · `FastMCP.as_proxy(...)` · `mcp.import_server(...)` · `mount(prefix=…)` (now `namespace=`) · `mcp.remove_tool(...)` · the `fastmcp.server.proxy` / `fastmcp.server.openapi` / `fastmcp.server.apps` import homes · `TaskConfig` in `server.tasks`. `Client("server.py")` is deprecated in favour of `Client(Path("server.py"))` because string sources are converging on URL semantics in FastMCP 5 (§35.11).
* **New first-class in v4** (§36.1): `InputRequiredResult` guards with `ctx.input_responses` and sealed `ctx.request_state` (§9.3, §38) · `UserSession` / `SessionId` / `SessionProvider` / `get_session()` (§11.3-§11.4, §40) · `ServerExtension` / `ClientExtension` / `add_extension()` / `MethodBinding` / `intercept_tool_call()` (§39) · background tasks as the negotiated `io.modelcontextprotocol/tasks` extension in the separate **`fastmcp-tasks`** package (§12) · `@mcp.completion` argument completion (§4.9, §41) · roles, insufficient-scope challenges, identity assertion (SEP-990, beta) and client-credentials auth (§42) · DI `CallArgument` binding (§10.3) · `ClientGroup` (§43).
* **MCP SDK v2 renamed every Python model field to snake_case** while the wire keeps camelCase aliases: `input_schema`, `output_schema`, `structured_content`, `is_error`, `mime_type`, `next_cursor`, `server_info`, `protocol_version` (§1.4, App. **B)**). `mcp_camelcase_compat` / `FASTMCP_MCP_CAMELCASE_COMPAT` bridges old reads with a deprecation warning — **turn it off in one CI job** to find what has not been ported (§35.2, §30).
* **Pydantic 2.13.4** (released 2026-05-06, supports Python ≥3.9) with **`pydantic-core` 2.46.4**, which Pydantic selects for itself — **never pin `pydantic-core` independently** (§1.3). Optional extras are only `[email]` and `[timezone]` (§1.1). 2.13.4 clears FastMCP 4's `>=2.12` floor.
* **`pydantic-settings` 2.15.0 is a separately versioned package, not an extra** (§38.0) — released 2026-08-07, Python ≥3.10. It has its own extras (§51.39) and its own source-priority model (§38.2, §39.1).
* **Pydantic 2.14.0b1 is prerelease and quarantined in §47** — it drops Python 3.9, adds initial 3.15 support, and changes model-build and core-schema behavior. Treat any 2.14 example as a migration event, not a drop-in. **The `fastmcp` reference no longer has a quarantine chapter**: v4 is the stable baseline, §35 is the 3→4 migration and §36 is the capability delta. Rule 13.
* **2.13-specific behavior an agent will otherwise get wrong** (§46, and the delta table at line 133): `polymorphic_serialization` is new and is the *narrow* alternative to broad `serialize_as_any` (§9.11, §19.3) · `exclude_if` now applies to computed fields (§20.2) · `StringConstraints(ascii_only=…)` is new (§11.2) · private-attribute default factories can receive validated model data (§6.5, §8.8) · discriminator-selected union branches no longer fall back across all members (§26.8) · extras assigned *after* init are now tracked in `model_fields_set` (§6.2) · and **2.13.1/2 restored `ValidationInfo.data`/`field_name` on the `model_validate_json` path** (§5.4, §33.7) — which is why 2.13.4 rather than 2.13.0.

> **`docs/library_ref/` still contains `fastmcp_python_advanced_reference_3.4.7.md`** (15,682 lines, §0-§37), the superseded v3 reference. A glob or grep across the directory will hit both files and the two disagree on nearly every chapter's content. **This skill routes only the 4.0.0 file**; open 3.4.7 solely to answer "what did v3 do?", and cite it explicitly as the v3 reference when you do. Rule 15.

---

## The two reference documents

Both live at `docs/library_ref/`. Each opens with a version anchor, a **documentation map**, a delta section and a source key, then deep-dives; each closes with a **dense appendix layer** that is the intended fast-lookup surface. Unlike the `gix`/`notify` pair, **neither document has an end-of-reference marker** — a chapter runs until the next `# … Advanced — N)` heading, and the last chapter runs to EOF.

| Doc | Path (`docs/library_ref/`) | Lines | Chapters | Deep-dive prefix | Subsection depth |
|-----|------|------:|---|------------------|---|
| **fastmcp** | `fastmcp_python_advanced_reference_4.0.0.md` | 12,692 | **§0-§44** | `# FastMCP Advanced — N) ` | **inconsistent** — 23 chapters list cleanly at `##` (§37 using letters instead of `N.M`), **11 number entirely at `###`**, 8 more demote only `N.0`, and §17/§22/§27 mix the two *and* hide content a third level down. Rule 2. |
| **pydantic** | `pydantic_python_advanced_reference_2.13.4.md` | 7,340 | **§0-§51** | `# Pydantic Advanced — N) ` | **uniform** `## N.M` throughout (594 `##` against 48 `###`). Whatever `--view expanded` shows is the whole chapter. |

**fastmcp §0-§44** — mental model, the three surfaces and **the protocol-era rule (§0)** · install, dependency floors, `httpx2`, `fastmcp-tasks`, `fastmcp-remote` (§1) · a full worked first server+client+test (§2) · **the v4 ownership map (§3: `FastMCP`, `Provider`, `Transform`, `ServerExtension`, `Client`, `ClientGroup`, `FastMCPApp`, sessions vs request state)** · server construction, extensions, completion, serving (§4) · **tools (§5 registration and the exact decorator surface, §6 typing/validation/outputs)** · resources and templates · prompts · **`Context` in v4 and what left it (§9)** · **dependency injection and `CallArgument` (§10)** · **the state taxonomy (§11)** · **tasks as an extension (§12)** · **middleware, now on a wider message surface (§13)** · **providers (§14)** · transforms, visibility, versioning, pagination · search, Code Mode, composition, proxying, gateways · **auth (§17)** and identity-aware security policy (§18) · running and deploying · HTTP hardening and scaling · **the programmatic client (§21-§22)** · client-only packaging · apps and interactive UI · Prefab and Generative UI · OpenAPI/FastAPI integration · **`fastmcp.json` and the CLI (§27-§28)** · observability · **testing and tool fingerprinting (§30)** · host integrations · security governance · performance and large catalogs · twelve production patterns · **3→4 migration (§35)** · **the capability delta and era matrix (§36)** · **9 lettered appendices (§37)** · then the seven dedicated v4 chapters: **multi-round guards (§38)** · **extensions (§39)** · **session state (§40)** · **completion (§41)** · **identity, roles and M2M auth (§42)** · **`ClientGroup` (§43)** · **the production/migration gate (§44)**.

**pydantic §0-§51** — mental model and eight core invariants · install and pinning · a worked first contract · **the architecture (§3: annotations → `CoreSchema` → Rust validator/serializer)** · `BaseModel` · **validation entry points (§5)** · trusted construction, copying, extras, field-set tracking, private state · **fields and `Annotated` (§7)** · defaults and factories · **`ConfigDict` (§9)** · **strict vs lax and the conversion contract (§10)** · reusable constrained types · **aliases (§12)** · field validators · model validators · functional validator metadata · **serialization (§16-§18)** · **polymorphic serialization and `SerializeAsAny` (§19)** · computed fields and lifecycle hooks · **`TypeAdapter` (§21)** · `RootModel` · dataclasses · `TypedDict`/`NamedTuple` · generics · **unions and discriminators (§26)** · forward refs and `model_rebuild` · dynamic models · **custom types and `CoreSchema` hooks (§29)** · standard, Pydantic-specific and network types · JSON parsing and partial validation · **JSON Schema (§34-§35)** · **errors (§36)** · `@validate_call` · **`pydantic-settings` (§38-§39)** · performance · experimental APIs · static typing · observability · framework boundaries · V1 migration · the 2.12→2.13.4 delta · the 2.14 boundary · testing · security · ten production patterns · **60 appendix subsections (§51)**.

**Reading strategy.** Start with `lib-outline <file>`, then `Read(offset, limit)` from REFERENCE.md §1. **The two docs read oppositely and want opposite tactics.**

`pydantic` chapters are **small** — median **~112** lines, range 73-234 once §51 is set aside — so read the whole chapter; it is usually cheaper than locating the right subsection. The exception dominates the document: **§51 is 1,264 lines, 17% of the file**, and it is the signature/matrix/cookbook layer. For "what is the exact 2.13.4 surface of `model_dump()`?" or "which exclusion flag?", **go to §51 first** (REFERENCE §1.4 maps its 60 subsections into four bands) and only fall back to the prose chapter for *why*.

`fastmcp` §0-§37 are **large** — median **~344** lines, up to 693 — so never read one whole; land on a subsection. §38-§44 are the opposite: **24-54 lines each, 228 lines for all seven**, so read those whole. `lib-outline --view expanded` only helps for some chapters: it is complete for **23**, complete-but-for-`N.0` for **8**, and returns **nothing at all for 11** (§0, §5-§8, §13, §14, §19, §21, §24, §29 — 3,640 lines, 28.7% of the document). Worst are §17, §22 and §27, where it returns a *partial* list that looks complete. For those, grep `^### N\.` instead. REFERENCE §1.1 carries the depth per chapter so you know before you look; Rule 2 explains it.

**Read the chapter's status banner before its subsections.** 28 of the 45 chapters — §0, §2, §5-§8 and every chapter from §13 to §34 — are the retained 3.4.7 topology carrying a bold **"FastMCP 4 status."** paragraph at the top that states that chapter's v4 delta. It sits *above* `N.0`, so a `Read` that seeks straight to the first numbered subsection skips it. The other 17 chapters (§1, §3, §4, §9-§12, §35-§44) were rewritten or are new and carry no banner. Rule 3.

---

## Where do I look?

| Symptom / question | Go to |
|---|---|
| "Where is *X* actually documented?" | REFERENCE **§2** — never grep; `Context` has 119 hits, `TypeAdapter` 103 |
| A v3 example no longer works | **fastmcp** §35 (the whole chapter) · App. **H)** anti-patterns · §36.2 era matrix |
| Which era am I on, and what does it cost me? | **fastmcp** §0 protocol-era rule, §3.6, §4.11 · **§36.2 the era matrix** |
| A tool needs to ask the user something | **fastmcp** §9.3 + **§38** (`InputRequiredResult`) · §9.4 for the legacy `ctx.elicit()` gate · App. **D)** |
| State has to survive between calls | **fastmcp** §11.0 taxonomy → §11.3 `UserSession` / §11.4 `SessionId` / §40 · App. **C)** |
| A tool argument is being coerced when it should not be | **fastmcp** §6.3 (`strict_input_validation`) → **pydantic** §10 (where strictness is set) |
| The generated tool input schema is wrong | **fastmcp** §6.1-§6.2, §6.4 → **pydantic** §7 (`Field`/`Annotated`), §34 (JSON Schema) |
| Structured output / `ToolResult` / content blocks | **fastmcp** §6.8-§6.13, §5.11 |
| A runtime value must not appear in the MCP schema | **fastmcp** §10 (DI), §6.5 · §10.3 when the hidden value depends on a public argument |
| Long-running work, progress, cancellation | **fastmcp** §12 (the tasks *extension*), §9.6 · App. **E)** |
| Adding a protocol capability of my own | **fastmcp** §39 `ServerExtension` · §3.5 · App. **F)** for extension-vs-middleware-vs-provider-vs-transform |
| Combining or proxying servers, or consuming several | **fastmcp** §14.18 `mount`, §14.19 `create_proxy`, **§43 `ClientGroup`** · App. **G)** |
| Which auth provider | **fastmcp** §17.1.1-§17.1.6 (a hidden `###` layer), §17.1.7 · **§42** for roles, scope challenges, identity assertion, M2M |
| Autocompleting a prompt argument or template parameter | **fastmcp** §41 · §4.9 for registration · §41.3 for the authorization caveat |
| Unknown input keys should fail / be kept / be dropped | **pydantic** §6.3, §9.2, §51.18 |
| Wire names differ from Python names | **pydantic** §12, §51.20 · **fastmcp** App. **B)** for the MCP model rename |
| A subclass is leaking fields into output | **pydantic** §19, §9.11, §51.26 |
| `None` vs missing in a PATCH body | **pydantic** §6.2, §6.8, §18.3, §8.6 |
| Validating a bare `list[int]` / union / `TypedDict` | **pydantic** §21, §51.27 |
| Reading config from env, `.env`, or a secret manager | **pydantic** §38-§39, §51.38, §51.40 · **fastmcp** §27 for `fastmcp.json` and `FASTMCP_*` |
| Validation or schema build is slow | **pydantic** §40, §51.42-§51.43 · **fastmcp** §33 |
| Verifying the published contract has not drifted | **fastmcp** §30.5-§30.9, `fastmcp inspect` (§27.11, §28.4) · **pydantic** §48.5 |
| Am I actually ready to ship v4? | **fastmcp** **§44** the migration gate · §35.12 the upgrade gate · App. **I)** |

---

## Key invariants

Taken from the documents themselves — `pydantic` §0.6 and §51.60, `fastmcp` §0.6, §36.3, §44.0 and App. **H)**/**I)**.

1. **Successful Pydantic validation describes the output, not the input.** Lax coercion is the default and is a feature; `M(x='123').x == 123`. Strictness is opt-in and settable at four levels (`pydantic` §0.6 inv. 1, §10).
2. **Validation and serialization are separate contracts.** Different aliases, different JSON Schemas (`mode='validation'` vs `'serialization'`), different behavior. Never assume one describes the other (`pydantic` §0.4, §34.2).
3. **Schema build is compile work; validation is the hot path.** Models and `TypeAdapter`s compile a Rust validator/serializer once. Constructing a `TypeAdapter` inside a loop or per request is the classic Pydantic performance bug (`pydantic` §0.6 inv. 5, §21.6, §40.2).
4. **Optionality and default are different.** `T | None` is *required and nullable* in V2. A default must be written (`pydantic` §0.6 inv. 4, §7.6).
5. **`model_construct()` skips validation and is a trust-boundary decision**, never a performance shortcut for external data (`pydantic` §0.6 inv. 3, §6.0).
6. **The FastMCP server object is identity + composition + policy, not transport.** The constructor does not own host/port/transport; `run()`/`http_app()`/the CLI bind it later (`fastmcp` §3.0, §4.0, §4.10).
7. **The modern era is sessionless, so cross-request continuity must be declared.** `ctx.set_state()` is request-local; a later call is a new request and inherits nothing. Cross-request state is `UserSession`, `SessionId`, or a domain store (`fastmcp` §9.2, §11.0, §35.9).
8. **A guard tool runs from the top on every round.** Multi-round interaction is a return value, not a suspended coroutine: middleware, authorization and the function body re-execute each leg, so the tool must be re-entrant and replay-safe, and irreversible side effects must be idempotently checkpointed (`fastmcp` §9.3, §38.4).
9. **Publication is not authorization.** Visibility filtering, tag filtering, Tool Search, tool annotations and `Approval` shape what a client *sees* or is *asked*; none of them is a security boundary. Neither is an extension advertisement, and completion is authenticated but not per-component authorized (`fastmcp` §15.22, §16.3, §18.7, §32.4, §32.14-§32.15, §41.3, App. **H)**).
10. **Anything shared across rounds, replicas or workers needs shared key material or storage.** Sealed `request_state` needs one `RequestStateSecurity` key ring on every replica; session state needs a shared store; task snapshots hold caller credentials and need `FASTMCP_TASKS_ENCRYPTION_KEY` (`fastmcp` §9.7, §11.6, §12.5, §38.3, §44.0).
11. **Pin exactly, and reconcile every example against the pin.** Both documents rank the installed package's own signatures above their own prose; the `fastmcp` reference additionally ranks PyPI and the official 3→4 upgrade guide above its own text (`fastmcp` §0 source precedence, §1.3 · `pydantic` §0.0, §47).
