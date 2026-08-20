# FastMCP in Python — advanced technical reference / feature-category catalog

This reference is designed for **LLM coding agents and engineers who need a source-oriented, implementation-level FastMCP guide**, rather than a quickstart. It uses the same pattern as the accompanying DataFusion-in-Rust reference: first establish a version/source anchor and exhaustive feature map, then make each numbered section a self-contained deep dive with call signatures, runtime semantics, deployment considerations, failure modes, anti-patterns, and agent checklists.

## Version / source anchors

**Primary deployable target: `fastmcp==3.4.7`**, the latest stable release as of **2026-08-19**. FastMCP 3.4.7 was released on 2026-08-10; PyPI declares **Python >=3.10** and exposes the extras `anthropic`, `apps`, `azure`, `code-mode`, `gemini`, `openai`, and `tasks`. The FastMCP release stream also contains **4.0.0b3**, released 2026-08-14, but that is a pre-release and is treated separately in §36 rather than silently mixed into stable examples. ([PyPI][FM-PYPI]) ([FastMCP Updates][FM-UPDATES])

The documentation site follows current development and may contain APIs marked for FastMCP 4 before FastMCP 4 reaches GA. Accordingly, this document uses the following precedence when sources disagree:

1. **PyPI / tagged stable release metadata** for the deployable package/version boundary.
2. **FastMCP 3.x documentation and stable-era SDK pages** for v3 behavior and signatures.
3. **Current official documentation** only when the page or feature is known to exist in 3.4.7, or when explicitly labeled as a v4 preview.
4. **GitHub release notes/changelog** for patch-level security, compatibility, and migration changes.
5. **Current `main`/v4 docs** only inside §36 unless a stable 3.x release note confirms the behavior.

This distinction is unusually important in August 2026. FastMCP 4 adopts the MCP `2026-07-28` sessionless protocol model and changes several server-side interaction primitives. A coding agent must not copy a v4-only example into a `fastmcp==3.4.7` deployment merely because it appears on the current docs site. ([FastMCP Updates][FM-UPDATES])

---

## Why this revision is materially broader than the prior FastMCP reference

The earlier FastMCP reference deliberately excluded installation and dependency setup and organized the framework into 20 major sections. It already captured the major v3 architecture—servers, clients, apps, providers/transforms, background tasks, authentication, CLI operations, and telemetry—but did not cover later 3.1–3.4 additions such as the `fastmcp-slim` client distribution, `fastmcp-remote`, Code Mode/search transforms in full depth, app-provider topology, 3.4-era HTTP/OAuth hardening, tool fingerprinting, or a stable-vs-v4 migration boundary as explicit first-class topics.

This revision expands the reference into **38 numbered deep dives** and adds the deployment foundation that the prior document intentionally omitted: installation and exact pins, project structure, first executable server/client/test patterns, state and storage architecture, visibility/versioning/pagination, OpenAPI/FastAPI generation, remote bridges, testing/contract fingerprinting, host integrations, scaling, security/governance, upgrade policy, and a dedicated FastMCP 4 transition guide.

---

## Feature inventory: what this reference covers

FastMCP’s stable 3.4.x surface is best understood as a framework stack rather than as a single decorator API:

```text
Python functions / Resource objects / Prompt functions / App providers
        │
        ├─ LocalProvider / FileSystemProvider / SkillsProvider / OpenAPIProvider
        ├─ mounted FastMCP providers / ProxyProvider / custom Provider
        │
        ▼
provider-local transforms
        │
        ▼
server aggregation
        │
        ▼
server transforms
        │
        ├─ Namespace
        ├─ Tool transformation
        ├─ visibility/version policy
        ├─ search / Code Mode
        ├─ resources-as-tools / prompts-as-tools
        │
        ▼
final visibility + authorization + pagination
        │
        ▼
MCP handlers + middleware
        │
        ├─ STDIO
        ├─ Streamable HTTP
        └─ legacy SSE
        │
        ▼
Client / CLI / host / app renderer
```

The reference covers the full authoring and operating surface around that pipeline:

* package installation, extras, version pinning, and project layout;
* server identity, lifecycle, decorators, and programmatic registration;
* tools, resources, templates, prompts, typing, schema generation, structured output, media blocks, errors, timeouts, and task execution;
* request `Context`, dependency injection, lifespan state, session state, persistent stores, progress, client logging, sampling, and elicitation for 3.x;
* providers, transforms, namespaces, visibility, versioning, pagination, tool search, Code Mode, OpenAPI providers, skills/filesystem providers, mounts, and proxies;
* authentication, authorization, OAuth/OIDC proxying, token verification, CIMD, client credentials, provider integrations, and HTTP trust boundaries;
* programmatic clients, transports, client auth, handlers, roots, notifications, progress, sampling/elicitation, and remote bridges;
* FastMCP Apps, Prefab, `FastMCPApp`, built-in app providers, Generative UI, and low-level custom HTML apps;
* `fastmcp.json`, settings/environment variables, CLI run/dev/list/call/inspect/generate/install flows;
* OpenTelemetry, inspection, deterministic in-memory tests, tool contract fingerprinting, and release regression gates;
* HTTP/ASGI deployment, FastAPI/Starlette mounting, reverse proxies, stateless HTTP, event stores, horizontal scaling, sandboxed agents, and remote MCP gateways;
* stable 3.x upgrade/migration guidance plus a deliberately isolated FastMCP 4 preview.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and FastMCP mental model

Define FastMCP as a **server + client + app framework** and establish the provider/transform execution model. Distinguish the framework from the MCP protocol itself, establish source-of-truth precedence, and explain the declaration → schema → provider → transform → middleware → transport → client path.

Deep dives include stable-vs-main docs policy, server/client/app ownership, local-vs-remote execution, and what FastMCP automates versus what application code must still own.

## 1) Installation, package selection, dependency policy, and project layout

Cover `uv add fastmcp`, `pip install fastmcp`, exact production pins, Python >=3.10, optional extras, `fastmcp-slim[client]`, `fastmcp-remote`, development dependencies, lockfiles, environment isolation, and package selection for servers versus clients.

Include `pyproject.toml` patterns, recommended repository layout, upgrade-safe pinning, and a stable/v4 prerelease install warning.

## 2) First executable server, client, and test

Provide canonical minimal programs for STDIO, HTTP, in-memory client invocation, tools/resources/prompts, and pytest. Clarify what starts a server, what only declares one, and the async lifecycle of `Client`.

## 3) Core API map and object model

Map `FastMCP`, `LocalProvider`, `Provider`, `Transform`, `Client`, `FastMCPApp`, tools/resources/prompts, and resolved execution methods. Make ownership and composition boundaries explicit.

## 4) Server construction and lifecycle

Deep-dive `FastMCP(...)`: identity, composition, behavior, handlers/storage, duplicate policy, strict validation, schema dereferencing, task defaults, lifespan, custom routes, and why transport configuration belongs at serve time in v3.

## 5) Tools: definition, registration, and execution contract

Cover `@mcp.tool`, standalone `@tool()`, `add_tool`, sync/async/thread execution, component metadata, `auth`, `task`, `timeout`, `version`, annotations, and the function-to-MCP call pipeline.

## 6) Tools: typing, validation, hidden parameters, outputs, and content blocks

Treat function signatures as public schemas. Cover Pydantic-driven input validation, `Annotated`/`Field`, strict vs flexible validation, `Depends`, `Context`, output schemas, `ToolResult`, media/file helpers, errors, timeouts, and structured/unstructured result channels.

## 7) Resources and resource templates

Cover fixed URI resources, function resources, concrete resource classes, RFC 6570 templates, wildcard/query parameters, MIME types, binary data, `ResourceResult`, context access, notifications, and template validation.

## 8) Prompts and prompt rendering

Cover `@mcp.prompt`, standalone prompt metadata for bound methods, typed/string-on-wire arguments, `Message`, `PromptResult`, versioning, runtime metadata, and render APIs.

## 9) MCP Context

Document request-scoped access to logging, progress, resources, prompts, sampling, elicitation, request/session metadata, lifespan state, session state, visibility changes, and task-aware behavior in FastMCP 3.x.

## 10) Dependency injection

Cover `Depends`, `CurrentContext`, `CurrentRequest`, `CurrentAccessToken`, `CurrentFastMCP`, nested dependencies, caching, scope, hidden parameters, and task-specific dependencies.

## 11) Lifespans, session state, storage, and state ownership

Separate four state domains: immutable declaration state, server-lifespan state, request state, and FastMCP 3 session state. Cover `session_state_store`, JSON-serializable state, distributed stores, restart/scaling semantics, and lifecycle cleanup.

## 12) Background tasks and long-running workflows

Cover `task=True`, task modes/configuration, Docket integration, workers, durable backends, task objects, status/progress/cancel/result flows, task auth scoping, and differences between foreground timeouts and background execution.

## 13) Middleware and the server policy layer

Deep-dive middleware ordering and hook layers, plus built-ins for logging, timing, caching, rate limiting, retries, errors, ping, response limiting, and cross-cutting policy. Call out auth/session-aware cache-key requirements.

## 14) Providers and dynamic component sources

Cover `LocalProvider`, `FileSystemProvider`, `SkillsProvider`, OpenAPI-backed providers, mounted FastMCP providers, `ProxyProvider`, custom providers, lookup precedence, lifespans, provider transforms, and immutable wrapping.

## 15) Transforms, visibility, versioning, pagination, and discovery shaping

Cover transform ordering, `Namespace`, tool transformations, resource/prompt bridges, enable/disable policy, semantic component keys, version selection/fallback, list pagination, and client-visible contract shaping.

## 16) Search transforms, Code Mode, composition, proxying, and gateways

Treat large-catalog discovery as a dedicated architectural topic. Cover regex/semantic search, Code Mode, `mount`, namespaces, proxy construction, transport bridges, remote composition, and gateway patterns.

## 17) Authentication and authorization

Cover token verification, remote OAuth metadata, OAuth/OIDC proxy patterns, full OAuth providers, `MultiAuth`, access-token injection, component `auth=`, scope checks, tag restrictions, and the HTTP-only nature of token auth.

## 18) Advanced security policy and identity-aware execution

Turn auth primitives into a production policy model: tenant isolation, least privilege, authorization-aware visibility, token audience/issuer constraints, proxy credential boundaries, client assertions/CIMD, backend token handling, and avoiding credential leakage through proxy/OpenAPI paths.

## 19) Running and deploying servers

Cover `run`, `run_async`, `run_http_async`, STDIO vs Streamable HTTP vs SSE, `http_app`, ASGI export, FastAPI/Starlette mounting, lifespan forwarding, route composition, and direct-vs-ASGI deployment decisions.

## 20) HTTP hardening, reverse proxies, scaling, and event delivery

Expand deployment beyond the basic runtime: `stateless_http`, stateful sessions in v3, Host/Origin protection, DNS-rebinding risk, SSRF protections, trusted corporate proxies, `.well-known` routes, mounted OAuth, reverse-proxy streaming, event stores, multiple workers, and load balancers.

## 21) Programmatic client fundamentals

Document `Client(...)` construction, transport inference, initialization, async context management, server capability inspection, calls/reads/prompts, result objects, and session reuse versus `client.new()`.

## 22) Client transports, handlers, roots, and client-side authentication

Cover STDIO, Streamable HTTP, SSE, in-memory transport, subprocess environments, OAuth/Bearer/CIMD/client credentials, roots, logging, progress, notifications, sampling and elicitation handlers, and timeout behavior.

## 23) Client-only packaging and `fastmcp-remote`

Cover the 3.3 `fastmcp-slim` split and the 3.4 `fastmcp-remote` bridge. Document when a host only supports STDIO, how remote OAuth is handled, header injection, endpoint mistakes, and why the bridge is different from `fastmcp run`.

## 24) Apps and interactive UI delivery

Cover MCP Apps as a distinct renderer plane, `app=True`, Prefab serialization, `FastMCPApp`, model-visible UI entry tools versus app-visible backend tools, renderer resources, and UI-to-MCP call routing.

## 25) Prefab, built-in app providers, Generative UI, and custom HTML

Deep-dive `Approval`, `Choice`, `FormInput`, `FileUpload`, `GenerativeUI`, Prefab component composition, dev preview, app security, CSP/resource concerns, and low-level MCP Apps custom HTML.

## 26) OpenAPI and FastAPI integration

Cover `FastMCP.from_openapi`, `FastMCP.from_fastapi`, `OpenAPIProvider`, route mapping, parameter/schema conversion, output validation, `deepObject`, server variables/base URLs, auth/header separation, and why generated MCP surfaces usually need curation/transforms.

## 27) Project configuration and settings

Treat `fastmcp.json` as a portable deployment contract and environment variables as runtime overrides. Cover `source`, environment/dependencies, deployment fields, CLI overrides, `.env`, `FASTMCP_...` settings, schema support, and config ownership.

## 28) CLI and developer workflows

Cover `fastmcp run`, reload/dev flows, Inspector, `list`, `call`, `discover` where applicable, `inspect`, `generate-cli`, host installation commands, auth utilities, MCP JSON generation, and agent-friendly shell workflows.

## 29) Observability, inspection, telemetry, and operational diagnostics

Cover OpenTelemetry spans, propagation, middleware timing, client logs, tool/resource/prompt inspection, server manifests, task observability, exception masking, and production trace boundaries.

## 30) Testing, contract verification, and tool fingerprinting

Cover in-memory `Client(server)` tests, pytest fixtures, `pytest-asyncio`, snapshots, schema assertions, auth/middleware tests, transport tests, version/visibility tests, tool fingerprinting, and contract-regression gates.

## 31) Ecosystem and host integrations

Map FastMCP integration patterns for ChatGPT, Claude Desktop/Code, Cursor, Gemini/Gemini CLI, Goose, OpenAI/Anthropic APIs, Pydantic AI, MCP JSON configurations, auth providers, and external policy services. Keep library semantics distinct from host-specific configuration.

## 32) Security hardening and governance checklist

Aggregate secure defaults and failure modes: HTTP auth boundaries, SSRF, DNS rebinding, redirect validation, path traversal/file uploads, CORS/origin policy, error masking, tool annotation vs enforcement, secrets, sandboxed agents, and governance around destructive/open-world tools.

## 33) Performance, scaling, resilience, and large-catalog engineering

Cover sync threadpool behavior, `run_in_thread=False`, tool timeouts, pagination, search transforms, caching, response limits, task backends, Redis/durable coordination, stateless HTTP, multi-worker deployment, catalog size, proxy latency, and schema/context token cost.

## 34) Production architecture patterns

Provide reference architectures for local desktop servers, internal HTTP services, multi-tenant gateways, OpenAPI adapters, sandboxed-agent capability boundaries, remote STDIO bridges, app-enabled servers, long-running task systems, and aggregated MCP gateways.

## 35) API stability, upgrade discipline, and FastMCP 2 → 3 migration

Cover exact version pins, release/change monitoring, decorator-mode changes, constructor/runtime split, providers/transforms, visibility/versioning, deprecated APIs, integration tests, and an upgrade checklist.

## 36) FastMCP 4 prerelease transition guide

Document **only as a preview**: sessionless MCP `2026-07-28`, protocol-era negotiation, `UserSession`/`SessionId`, multi-round interactive tools, extension APIs, completion handlers, v4 task packaging, identity assertions/roles, and the removal of 3.x server-initiated sampling/roots. Keep v4 examples out of stable sections.

## 37) Dense appendices and lookup matrices

Finish with high-speed reference units: decorator matrices, DI matrix, return conversion tables, resource-template grammar, provider/transform comparison, task truth table, transport/auth decision tables, deployment invariants, CLI quick reference, error taxonomy, version feature gates, and a production readiness checklist.

---

# Stable release delta — what changed after FastMCP 3.0

For agents upgrading older v3 references, these are the release families that materially affect architecture rather than only patch behavior:

| Release | Architectural significance for this guide |
|---|---|
| **3.1.0** | Code Mode, tool-search transforms, `MultiAuth`, early Apps integration. |
| **3.2.0** | FastMCP Apps becomes a major product surface; built-in `FileUpload`, `Approval`, `Choice`, `FormInput`, and `GenerativeUI` providers. |
| **3.2.4** | Background task ownership/auth scoping hardened; docstring parameter descriptions and security fixes. |
| **3.3.0** | `fastmcp-slim` client-focused distribution; `run_in_thread=False`; OAuth/telemetry hardening. |
| **3.4.0** | `fastmcp-remote`; proxy initialization correctness; richer `ToolResult(..., is_error=True)` handling. |
| **3.4.3–3.4.7** | SSRF, Host/Origin, redirect, JWKS, trusted-proxy, OAuth/CIMD compatibility and security hardening. |

Patch releases matter in a framework that sits on an authentication and network boundary. Do not pin “3.4” loosely in long-lived production infrastructure and assume all 3.4 patch behavior is equivalent; review security/compatibility notes before upgrades.

---

# Source index used throughout this reference

[FM-PYPI]: https://pypi.org/project/fastmcp/ "FastMCP on PyPI"
[FM-UPDATES]: https://gofastmcp.com/updates "FastMCP Updates"
[FM-WELCOME]: https://gofastmcp.com/getting-started/welcome "FastMCP framework overview"
[FM-INSTALL]: https://gofastmcp.com/getting-started/installation "Installation"
[FM-SERVER]: https://gofastmcp.com/servers/server "FastMCP Server"
[FM-TOOLS]: https://gofastmcp.com/servers/tools "Tools"
[FM-RESOURCES]: https://gofastmcp.com/servers/resources "Resources and Templates"
[FM-PROMPTS]: https://gofastmcp.com/servers/prompts "Prompts"
[FM-CONTEXT]: https://gofastmcp.com/servers/context "Context"
[FM-DI]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection"
[FM-LIFESPAN]: https://gofastmcp.com/servers/lifespan "Lifespans"
[FM-TASKS]: https://gofastmcp.com/servers/tasks "Background Tasks"
[FM-MIDDLEWARE]: https://gofastmcp.com/servers/middleware "Middleware"
[FM-PROVIDERS]: https://gofastmcp.com/servers/providers/overview "Providers"
[FM-TRANSFORMS]: https://gofastmcp.com/servers/transforms/transforms "Transforms"
[FM-SEARCH]: https://gofastmcp.com/servers/transforms/tool-search "Tool Search"
[FM-CODEMODE]: https://gofastmcp.com/servers/transforms/code-mode "Code Mode"
[FM-VISIBILITY]: https://gofastmcp.com/servers/visibility "Visibility"
[FM-VERSIONING]: https://gofastmcp.com/servers/versioning "Versioning"
[FM-PAGINATION]: https://gofastmcp.com/servers/pagination "Pagination"
[FM-COMPOSITION]: https://gofastmcp.com/servers/composition "Composition"
[FM-AUTHN]: https://gofastmcp.com/servers/auth/authentication "Authentication"
[FM-AUTHZ]: https://gofastmcp.com/servers/authorization "Authorization"
[FM-HTTP]: https://gofastmcp.com/deployment/http "HTTP Deployment"
[FM-RUNNING]: https://gofastmcp.com/deployment/running-server "Running Servers"
[FM-CLIENT]: https://gofastmcp.com/clients/client "FastMCP Client"
[FM-TRANSPORTS]: https://gofastmcp.com/clients/transports "Client Transports"
[FM-REMOTE]: https://gofastmcp.com/clients/fastmcp-remote "fastmcp-remote"
[FM-APPS]: https://gofastmcp.com/apps/overview "FastMCP Apps"
[FM-OPENAPI]: https://gofastmcp.com/integrations/openapi "OpenAPI Integration"
[FM-FASTAPI]: https://gofastmcp.com/integrations/fastapi "FastAPI Integration"
[FM-CONFIG]: https://gofastmcp.com/deployment/server-configuration "Project Configuration"
[FM-SETTINGS]: https://gofastmcp.com/more/settings "Settings"
[FM-CLI]: https://gofastmcp.com/cli/overview "CLI"
[FM-TELEMETRY]: https://gofastmcp.com/servers/telemetry "OpenTelemetry"
[FM-TESTING]: https://gofastmcp.com/servers/testing "Testing"
[FM-FINGERPRINT]: https://gofastmcp.com/servers/tool-fingerprinting "Tool Fingerprinting"
[FM-UPGRADE3]: https://gofastmcp.com/getting-started/upgrading/from-fastmcp-3 "Upgrading from FastMCP 3"
[FM-UPGRADE2]: https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2 "Upgrading from FastMCP 2"

---

# FastMCP Advanced — 0) Scope, versioning, and mental model
### Scope, versioning, and the FastMCP mental model

### Version anchors used here

FastMCP’s top-level docs are **not** a frozen release snapshot: the site explicitly says it reflects the `main` branch, version badges mark when a feature was introduced, and unreleased features may already appear in the docs. Separately, the SDK reference is auto-generated from source docstrings and type annotations on every merge to `main`. For agent-authored documentation, that means: treat top-level prose as **forward-looking**, treat version badges as **feature gates**, and treat SDK pages as the highest-confidence source for current call signatures. ([FastMCP][1])

FastMCP also ships unusually agent-friendly docs surfaces: the docs are exposed through an MCP server, available as `llms.txt` / `llms-full.txt`, and any page can be fetched as markdown by appending `.md`. For LLM programming agents, that materially changes the retrieval strategy: prefer API reference pages for signatures, prose pages for semantics, and the markdown/LLM endpoints for bulk ingestion or offline synthesis. ([FastMCP][1])

### 0.1 Compression of the system model

FastMCP is best modeled as a **three-surface framework**: **Servers** publish MCP capabilities, **Clients** consume them through a deterministic typed interface, and **Apps** add optional interactive UI rendering directly inside the host conversation. The official docs present those three pillars explicitly: servers wrap Python functions into MCP tools/resources/prompts, clients connect to any server with full protocol support, and apps render interactive UIs in-conversation. ([FastMCP][1])

For implementation work, the higher-precision mental model is:

1. **Declaration layer** — Python functions, type hints, docstrings, metadata.
2. **Compilation layer** — FastMCP derives MCP schema, validation rules, descriptions, and component metadata.
3. **Runtime layer** — transport, auth, lifecycle, middleware, session management, routing.
4. **Consumption layer** — a `Client` performs explicit MCP operations.
5. **Optional rendering layer** — app metadata causes tool results to become interactive UIs rather than plain JSON/text. ([FastMCP][1])

That decomposition matters because **Apps are not a separate transport** and **Clients are not a code generator**. Apps are layered on top of server-declared tools; Clients are explicit MCP consumers. The server is still the authoritative source of capability definitions and metadata. ([FastMCP][2])

### 0.2 Surface 1 — Servers

`FastMCP` is the central server object. The docs define it as the container for tools, resources, and prompts, and the constructor is organized into identity, composition, behavior, and handler/storage concerns. At minimum, a server needs a `name`; `instructions` are surfaced to clients/LLMs; `version` defaults to the FastMCP library version if not set. ([FastMCP][2])

Server-side MCP components are semantically distinct:

* **Tools**: invocable functions for action or external-system access.
* **Resources**: passive readable data sources.
* **Prompts**: reusable message templates. ([FastMCP][2])

The crucial authoring invariant is that FastMCP wants **ordinary Python declarations** as the source of truth. For tools specifically, the docs say that declaring a Python function gives you automatic schema, validation, and documentation generation, and that tool execution follows a predictable pipeline: the LLM/client sends arguments, FastMCP validates against the function signature, the function runs, and the result is returned. ([FastMCP][1])

### 0.3 Surface 2 — Clients

`fastmcp.Client` is a **programmatic**, **deterministic**, **explicit** MCP consumer. The docs stress that it is not an autonomous agent runtime; it is a typed interface for controlled interactions with any MCP server. That makes it the correct primitive for integration tests, deterministic orchestration, adapters, evaluators, and higher-level agent frameworks that want MCP transport/auth/protocol handling without surrendering control flow. ([FastMCP][3])

Client construction is intentionally broad: the client can infer transport from an in-process `FastMCP` server, a URL, a local script path, config objects, or transport instances. Entering `async with client:` initializes the MCP session automatically unless `auto_initialize=False` was configured; `initialize()` performs the MCP handshake explicitly and returns capabilities, protocol version, server info, and optional instructions. ([FastMCP][3])

Operationally, the client surface is the “consumer half” of the framework: `ping()`, `list_tools()`, `list_resources()`, `list_prompts()`, `call_tool(...)`, and related methods are the canonical way to validate what the server actually published over transport, as opposed to what the Python module appears to declare locally. ([FastMCP][3])

### 0.4 Surface 3 — Apps

Apps are the optional UI plane. The official overview is explicit: most apps start by adding `app=True` to a tool and returning Prefab components; when the UI needs multiple backend tools with managed visibility and composition safety, use `FastMCPApp`; when full control is required, use custom HTML via the MCP Apps extension. ([FastMCP][4])

The highest-value mental-model compression is: **an app is still a tool result**, but the result carries renderer metadata and `structuredContent` instead of being only plain text or JSON. The architecture docs give the pipeline exactly: **Python components → JSON tree → structuredContent → Renderer iframe → Host UI**. The host loads a renderer in a sandboxed iframe, pushes the structured payload into it, and UI-originated server calls travel back through `postMessage` into normal MCP `tools/call` requests. ([FastMCP][5])

`FastMCPApp` is not just syntactic sugar. It is a provider that binds **entry-point tools** (`@app.ui()`) and **backend tools** (`@app.tool()`), tags them with app identity, manages visibility (`model` vs `app`), and preserves backend-tool resolution even through transforms/namespacing. That is the right abstraction once a UI stops being a single display tool and starts behaving like a composable application. ([FastMCP][6])

### 0.5 Canonical end-to-end flow

#### Stage A — Python declaration

Server declaration begins with `FastMCP(...)`, then component registration with decorators such as `@mcp.tool`, `@mcp.resource(...)`, and `@mcp.prompt`. Minimal server syntax is intentionally small. ([FastMCP][2])

```python
from fastmcp import FastMCP

mcp = FastMCP(
    "DataAnalysis",
    instructions="Provides tools for analyzing numerical datasets. Start with get_summary() for an overview.",
)

@mcp.tool
def add(a: int, b: int) -> int:
    """Add two integers."""
    return a + b

@mcp.resource("data://config")
def get_config() -> dict:
    return {"theme": "dark", "version": "1.0"}

@mcp.prompt
def analyze_data(data_points: list[float]) -> str:
    return f"Analyze: {data_points}"
```

That skeleton matches the documented minimal constructor and the three primary server component types. ([FastMCP][2])

#### Stage B — MCP schema / validation / docs generation

FastMCP’s value proposition at declaration time is that the Python signature becomes the MCP contract. The docs say the framework auto-generates schema, validation, and documentation from Python functions, and the tool execution flow explicitly validates request parameters against the function signature before your implementation runs. ([FastMCP][1])

For agent authors, the important consequence is: **the Python callable is the canonical schema source**, not a parallel JSON schema file. If your signature, docstring, or return annotation is wrong, the generated MCP contract is wrong. If those are correct, FastMCP will usually out-document a hand-written wrapper because the protocol description remains mechanically aligned with the code. ([FastMCP][1])

#### Stage C — Transport exposure

Publication happens through `mcp.run(...)` or `mcp.http_app()`. `mcp.run()` is synchronous and supports `stdio`, `http`, `sse`, and `streamable-http` transport names; the server docs recommend guarding it with `if __name__ == "__main__":` because MCP clients may launch the server as a subprocess. STDIO is the default; HTTP uses Streamable HTTP; SSE is documented as the legacy/deprecated web transport. ([FastMCP][7])

```python
from fastmcp import FastMCP

mcp = FastMCP("MyServer")

@mcp.tool
def greet(name: str) -> str:
    return f"Hello, {name}!"

if __name__ == "__main__":
    mcp.run()  # STDIO by default
    # or: mcp.run(transport="http", host="127.0.0.1", port=9000)
```

This is the documented publication pattern for local subprocess and simple remote deployment. ([FastMCP][2])

#### Stage D — Client invocation

The client consumes the published MCP surface via an explicit async lifecycle. It auto-inferrs transport from the source, initializes on context entry by default, and then exposes MCP operations directly. ([FastMCP][3])

```python
import asyncio
from fastmcp import Client

async def main():
    async with Client("https://example.com/mcp") as client:
        await client.ping()
        tools = await client.list_tools()
        resources = await client.list_resources()
        prompts = await client.list_prompts()
        result = await client.call_tool("example_tool", {"param": "value"})
        print(result)

asyncio.run(main())
```

This matches the documented client usage pattern and is the right reference shape for tests, orchestration code, and adapter layers. ([FastMCP][3])

#### Stage E — Optional interactive UI rendering

A tool becomes an app entry point either by setting `app=True` or by using `FastMCPApp`/`@app.ui()`. For `app=True`, FastMCP can expand that flag into full app metadata, link the tool to the shared renderer resource, and serialize Prefab output into `structuredContent`. If the return annotation is already a Prefab type, FastMCP can auto-wire app metadata even without an explicit `app=True`. ([FastMCP][4])

```python
from fastmcp import FastMCP
from prefab_ui.app import PrefabApp
from prefab_ui.components import Column, Heading

mcp = FastMCP("Dashboard")

@mcp.tool(app=True)
def revenue_chart(year: int) -> PrefabApp:
    with Column() as view:
        Heading(f"Revenue for {year}")
    return PrefabApp(view=view)
```

That is the shortest correct mental model for “tool result renders as UI instead of plain JSON.” ([FastMCP][4])

When the app surface grows beyond a single UI-returning tool, the documented promoted pattern is `FastMCPApp`:

```python
from fastmcp import FastMCP, FastMCPApp
from prefab_ui.app import PrefabApp

app = FastMCPApp("Notes")

@app.tool()
def add_note(title: str, body: str) -> list[dict]:
    ...

@app.ui()
def notes_app() -> PrefabApp:
    ...

mcp = FastMCP("Notes Server", providers=[app])
```

This is the right abstraction when the model-visible entry point and the UI-only backend tools must remain composition-safe under namespaces and transforms. ([FastMCP][8])

### 0.6 Deployment invariants and best-practice advisory

**Local / desktop / subprocess-hosted MCP**: prefer `mcp.run()` with default STDIO. The running-server docs explicitly position STDIO for local development, Claude Desktop, CLI tools, and single-user applications. Always keep the `__main__` guard for subprocess-launched environments. ([FastMCP][9])

**Remote / multi-client / service deployment**: prefer Streamable HTTP, not SSE. The docs say HTTP transport provides full bidirectional communication, supports all MCP operations including streaming responses, and allows multiple clients to share one server process; SSE is called legacy/deprecated. ([FastMCP][9])

**Production ASGI hosting**: prefer `mcp.http_app()` when you need Uvicorn control, multiple workers, custom middleware, or integration with existing Starlette/FastAPI applications. When mounting, pass `lifespan=mcp_app.lifespan`; the docs are explicit that nested lifespans are not recognized automatically and session management will fail without forwarding FastMCP’s lifespan. ([FastMCP][10])

**Mounted authenticated HTTP deployments**: keep `base_url` and `mcp_path` distinct, and treat `base_url + mcp_path` as the actual externally reachable MCP URL. If the server is OAuth-protected and mounted under a prefix, `.well-known` discovery routes must stay at the root level; FastMCP cannot infer that automatically when the app is mounted under a prefix. ([FastMCP][10])

**Apps in production**: treat the app layer as the fastest-moving surface. The FastMCPApp docs explicitly warn that Prefab is in early active development and can break across releases; pin `prefab-ui` to a specific version. For local previewing, `fastmcp dev apps` provides a browser-based host simulation without needing a real MCP host client. ([FastMCP][8])

### 0.7 Value case, by surface

**Servers** are the leverage surface. Their value is that a normal Python declaration becomes an MCP-compliant capability surface without hand-maintaining parallel schemas, validators, and descriptions. That collapses authoring cost and reduces schema drift. ([FastMCP][1])

**Clients** are the verification and integration surface. Their value is deterministic, typed protocol interaction: exact control over handshake, listings, calls, and lifecycle, suitable for tests and production orchestration rather than only conversational agent loops. ([FastMCP][3])

**Apps** are the presentation surface. Their value is that tool outputs can become usable human interfaces—charts, forms, dashboards, approval steps—without abandoning the MCP model. For simple display, `app=True` is enough; for multi-tool UI systems, `FastMCPApp` adds visibility discipline, stable backend identifiers, and namespace-safe routing. ([FastMCP][4])

### 0.8 Authoring policy for the rest of the documentation set

For all subsequent FastMCP deep dives, use this policy:

* Treat **Python declaration syntax** as the primary contract surface. ([FastMCP][1])
* Treat **SDK reference pages** as the signature source and prose pages as the behavioral source. This follows the docs’ own auto-generation model. ([FastMCP][11])
* Treat **top-level docs as potentially ahead of release**; gate every feature by badge/version, not by page presence alone. ([FastMCP][1])
* Annotate every feature later with at least three dimensions: **introduced version**, **transport scope** (`stdio` vs HTTP vs app-only), and **deployment scope** (local / remote / mounted / auth-sensitive). Those dimensions are repeatedly material in the official docs. ([FastMCP][1])
* For LLM-agent retrieval, prefer **markdown pages**, **llms-full.txt**, or the docs’ own MCP server when building automated synthesis pipelines. ([FastMCP][1])


[1]: https://gofastmcp.com/getting-started/welcome "Welcome to FastMCP - FastMCP"
[2]: https://gofastmcp.com/servers/server "The FastMCP Server - FastMCP"
[3]: https://gofastmcp.com/clients/client "The FastMCP Client - FastMCP"
[4]: https://gofastmcp.com/apps/overview "Apps - FastMCP"
[5]: https://gofastmcp.com/apps/architecture "App Architecture - FastMCP"
[6]: https://gofastmcp.com/python-sdk/fastmcp-apps-app "app - FastMCP"
[7]: https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport "transport - FastMCP"
[8]: https://gofastmcp.com/apps/interactive-apps "FastMCPApp - FastMCP"
[9]: https://gofastmcp.com/deployment/running-server "Running Your Server - FastMCP"
[10]: https://gofastmcp.com/deployment/http "HTTP Deployment - FastMCP"
[11]: https://gofastmcp.com/development/contributing "Contributing - FastMCP"

# FastMCP Advanced — 1) Installation, package selection, dependency policy, and project layout

## 1.0 Release target and install policy

This reference targets **FastMCP 3.4.7**. For production, CI, or any generated code that must remain reproducible, pin the version explicitly rather than installing an unconstrained `fastmcp` and assuming the same API will be present later.

```bash
# Recommended with uv
uv add "fastmcp==3.4.7"

# Equivalent with pip
python -m pip install "fastmcp==3.4.7"
```

FastMCP's current installation guide states that an unversioned install resolves to the latest **stable** release while FastMCP 4 remains in prerelease. PyPI metadata for 3.4.7 requires Python 3.10 or newer. ([FastMCP Installation][1]) ([PyPI][2])

**Agent rule:** never infer the deployable FastMCP major version from the current documentation landing page. Resolve the installed version first, then use the corresponding API surface.

```bash
fastmcp version
python -c 'import fastmcp; print(fastmcp.__version__)'
```

---

## 1.1 Python version and interpreter policy

PyPI declares:

```text
Requires-Python: >=3.10
```

For application repositories, declare the same boundary—or a narrower one you actually test—in `pyproject.toml`.

```toml
[project]
name = "my-mcp-server"
version = "0.1.0"
requires-python = ">=3.10,<3.14"
dependencies = [
  "fastmcp==3.4.7",
]
```

The upper bound is optional; use one when your deployment fleet or transitive libraries have not yet been validated on newer Python versions. FastMCP 3.4.7 advertises Python 3.10–3.13 classifiers. ([PyPI][2])

**Do not** make the runtime interpreter an implicit workstation detail. STDIO clients launch server processes themselves, and a wrong interpreter is one of the most common reasons a server works in a shell but fails in a host application.

---

## 1.2 Full package vs optional extras

The full package is the default server/client distribution:

```bash
uv add "fastmcp==3.4.7"
```

PyPI exposes these 3.4.7 extras:

```text
anthropic
apps
azure
code-mode
gemini
openai
tasks
```

Use extras deliberately:

```bash
# background tasks / Docket integration
uv add "fastmcp[tasks]==3.4.7"

# interactive app surface
uv add "fastmcp[apps]==3.4.7"

# Code Mode transform
uv add "fastmcp[code-mode]==3.4.7"

# server-side sampling handler integrations when needed
uv add "fastmcp[openai]==3.4.7"
uv add "fastmcp[anthropic]==3.4.7"
```

Do not install every extra “just in case.” Extras enlarge the dependency graph, attack surface, lockfile churn, and potential version conflicts. ([PyPI][2])

---

## 1.3 Client-only footprint: `fastmcp-slim`

FastMCP 3.3 introduced a dependency-light distribution for code that only needs the client/transport side. Current documentation exposes the client-only install through the normal `fastmcp` import namespace:

```bash
pip install "fastmcp-slim[client]"
```

```python
from fastmcp import Client

client = Client("https://example.com/mcp")
```

Use this for:

* libraries embedding an MCP client but not hosting a FastMCP server;
* agent runtimes that connect to MCP endpoints;
* CI or integration harnesses that do not need Starlette/Uvicorn/server dependencies;
* applications where minimizing transitive server dependencies matters.

Use the **full package** when your process defines or runs `FastMCP`, uses Apps/server providers, or needs other server-only features. FastMCP 3.3 specifically introduced the slim distribution to separate this footprint. ([FastMCP Updates][3]) ([Client-Only Package][4])

### Import invariant

The package split does **not** imply a new Python namespace:

```python
# Correct with full fastmcp or the client-only distribution
from fastmcp import Client
```

Do not generate `from fastmcp_slim import Client`.

---

## 1.4 Remote bridge package: `fastmcp-remote`

FastMCP 3.4 introduced a separate executable package for **stdio-only hosts that need to connect to a remote HTTP MCP server**.

Typical host command:

```bash
uvx fastmcp-remote https://example.com/mcp
```

Typical MCP host configuration:

```json
{
  "mcpServers": {
    "remote-api": {
      "command": "uvx",
      "args": [
        "fastmcp-remote",
        "https://example.com/mcp"
      ]
    }
  }
}
```

This is a bridge process, not the normal server runner. Use:

```text
fastmcp run       -> run a Python/FastMCP server you own
fastmcp-remote    -> bridge a remote HTTP MCP server into local STDIO
Client(url)       -> connect programmatically from Python
```

FastMCP 3.4.0 made `fastmcp-remote` a first-class release feature and hardened proxy initialization so an unreachable/wrong upstream fails during initialization rather than looking like an empty server. ([FastMCP Updates][3]) ([fastmcp-remote][5])

---

## 1.5 Development dependency set

A practical dev/test group:

```toml
[dependency-groups]
dev = [
  "pytest>=8",
  "pytest-asyncio>=0.24",
  "inline-snapshot>=0.20",
  "dirty-equals>=0.9",
  "ruff>=0.12",
  "mypy>=1.16",
]
```

FastMCP's testing guide recommends `pytest-asyncio` for async client tests and highlights inline snapshots for complex protocol objects. ([FastMCP Testing][6])

Recommended pytest configuration:

```toml
[tool.pytest.ini_options]
asyncio_mode = "auto"
```

This keeps FastMCP client tests concise:

```python
async def test_server(client):
    tools = await client.list_tools()
    assert tools
```

---

## 1.6 Minimal repository layout

Use a layout that separates MCP declarations from domain logic and deployment glue.

```text
my-mcp-server/
├── pyproject.toml
├── uv.lock
├── fastmcp.json                 # optional portable run/deploy contract
├── src/
│   └── my_server/
│       ├── __init__.py
│       ├── server.py            # FastMCP construction / registrations
│       ├── tools/
│       │   ├── __init__.py
│       │   ├── search.py
│       │   └── write.py
│       ├── resources/
│       │   └── catalog.py
│       ├── prompts/
│       │   └── analysis.py
│       ├── dependencies.py      # Depends providers
│       ├── lifespan.py          # startup/shutdown resources
│       ├── auth.py
│       ├── middleware.py
│       └── settings.py
└── tests/
    ├── conftest.py
    ├── test_tools.py
    ├── test_resources.py
    ├── test_auth.py
    └── test_contract.py
```

**Boundary rule:** the MCP layer should adapt domain/application services; domain services should not need to know they are running under FastMCP.

---

## 1.7 `pyproject.toml` — server project pattern

```toml
[project]
name = "inventory-mcp"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = [
  "fastmcp==3.4.7",
  "pydantic>=2.11,<3",
  "httpx>=0.28,<1",
]

[dependency-groups]
dev = [
  "pytest>=8",
  "pytest-asyncio>=0.24",
  "inline-snapshot>=0.20",
]

[tool.pytest.ini_options]
asyncio_mode = "auto"

[tool.ruff]
target-version = "py311"
```

When task execution is required:

```toml
[project]
dependencies = [
  "fastmcp[tasks]==3.4.7",
]
```

When Apps are required:

```toml
[project]
dependencies = [
  "fastmcp[apps]==3.4.7",
]
```

---

## 1.8 Lockfiles and exact pins

Recommended production posture:

```bash
uv lock
uv sync --frozen
```

or, with a requirements workflow:

```bash
pip-compile pyproject.toml
python -m pip install --require-hashes -r requirements.txt
```

Why exact FastMCP pins matter more than usual:

* the MCP protocol itself evolves;
* FastMCP closely tracks protocol behavior;
* auth/transport patches can materially change secure deployment semantics;
* current docs may lead stable releases;
* v4 prereleases coexist with stable v3 releases.

A lockfile is not a substitute for a documented top-level FastMCP version target. Record both.

---

## 1.9 Stable vs prerelease installation

As of this reference:

```text
stable target:    3.4.7
v4 prerelease:    4.0.0b3
```

Normal installs should remain stable:

```bash
uv add "fastmcp==3.4.7"
```

Do not add a broad prerelease constraint such as:

```bash
# Avoid in normal production environments
pip install --pre fastmcp
```

If intentionally testing v4, isolate it in a branch/environment and pin the exact beta:

```bash
uv venv .venv-v4
uv pip install "fastmcp==4.0.0b3"
```

Then run the migration suite in §36. ([FastMCP Installation][1])

---

## 1.10 Environment isolation for STDIO servers

A local host may launch your MCP server with a very different environment than your shell. Prefer explicit commands and project roots.

Example host config:

```json
{
  "mcpServers": {
    "inventory": {
      "command": "uv",
      "args": [
        "--directory",
        "/absolute/path/to/inventory-mcp",
        "run",
        "python",
        "-m",
        "my_server.server"
      ]
    }
  }
}
```

Avoid relying on:

* shell aliases;
* interactive-shell PATH mutations;
* implicit current working directory;
* unexported `.env` values;
* whichever `python` happens to be first in PATH.

For client-side `StdioTransport`, also treat the child environment as explicit configuration; do not assume every shell variable is inherited in the way a terminal session would provide it.

---

## 1.11 Configuration ownership: package vs `fastmcp.json` vs environment

Use three layers deliberately:

```text
pyproject.toml / lockfile
    -> dependency/runtime package contract

fastmcp.json
    -> portable FastMCP project/run/deployment contract

FASTMCP_* environment variables / .env
    -> environment-specific operational overrides
```

Do not encode secrets in `fastmcp.json` or source-controlled Python. Put secret values into the deployment secret store/environment and reference them through your settings/dependency layer.

---

## 1.12 Package verification and smoke check

After environment creation:

```bash
fastmcp version
python - <<'PY'
from fastmcp import Client, FastMCP

mcp = FastMCP("smoke")

@mcp.tool
def add(a: int, b: int) -> int:
    return a + b

print(mcp.name)
print(Client(mcp))
PY
```

For a server repository, add a CI gate that imports every module that performs registrations. Many failures—missing optional extras, bad future annotations, stale imports—can be caught before runtime with a simple import test.

---

## 1.13 Dependency extras decision table

| Need | Install stance | Notes |
|---|---|---|
| Build/run server | `fastmcp==3.4.7` | default full framework |
| Client only | `fastmcp-slim[client]` | dependency-light client/transport layer |
| Background tasks | `fastmcp[tasks]==3.4.7` | Docket/task dependencies |
| Apps | `fastmcp[apps]==3.4.7` | interactive UI dependencies |
| Code Mode | `fastmcp[code-mode]==3.4.7` | sandbox/code-mode dependencies |
| OpenAI sampling handler | `fastmcp[openai]==3.4.7` | only if using that handler path |
| Anthropic sampling handler | `fastmcp[anthropic]==3.4.7` | only if using that handler path |
| Remote stdio bridge | `uvx fastmcp-remote ...` | separate package/executable |
| Test only | full/slim + `pytest-asyncio` | use in-memory client where possible |

---

## 1.14 Agent installation invariants

1. **Resolve the installed FastMCP version before choosing APIs.**
2. **Pin stable 3.4.7 in code intended for this reference.**
3. **Never copy v4-only API shapes into stable 3.x code.**
4. **Use the full package for servers; consider slim for client-only libraries.**
5. **Use optional extras only when their feature is actually needed.**
6. **Treat the interpreter path and working directory as explicit for STDIO hosts.**
7. **Keep domain logic independent of FastMCP imports wherever practical.**
8. **Commit a lockfile for applications.**
9. **Keep secrets out of Python source and portable project config.**
10. **Run `fastmcp version` in CI/deployment diagnostics.**

---

## 1.15 Anti-pattern inventory

* `pip install fastmcp` in a Docker build with no lockfile or version constraint.
* using a v4 beta because the current docs showed a newer API.
* adding `tasks`, `apps`, `openai`, `anthropic`, and `code-mode` extras to every server by default.
* using `fastmcp-slim` to host a FastMCP server without verifying server dependencies are present.
* treating `fastmcp-remote` as the way to run a local Python server.
* placing all tools, dependencies, auth, middleware, and startup code in one `server.py` as the project grows.
* hardcoding production tokens into host JSON files committed to source control.
* assuming a desktop MCP host uses the same virtualenv/PATH as your shell.
* depending on unpinned pre-release transitive packages in an authentication-sensitive server.
* mixing a 3.4.7 runtime with code generated from current v4-only server session/elicitation examples.

---

## 1.16 Installation checklist

```text
[ ] Pin fastmcp==3.4.7 for this reference.
[ ] Verify Python >=3.10 and the deployment interpreter actually used by the MCP host.
[ ] Choose full package vs client-only slim deliberately.
[ ] Add only required extras.
[ ] Use fastmcp-remote only for remote-HTTP -> local-STDIO bridging.
[ ] Commit uv.lock / equivalent application lockfile.
[ ] Add pytest-asyncio and configure asyncio_mode=auto.
[ ] Separate server declarations from domain logic.
[ ] Add an import/smoke test to CI.
[ ] Run `fastmcp version` in deployment diagnostics.
[ ] Keep v4 beta testing in a separate environment/branch.
```

[1]: https://gofastmcp.com/getting-started/installation "FastMCP Installation"
[2]: https://pypi.org/project/fastmcp/ "FastMCP on PyPI"
[3]: https://gofastmcp.com/updates "FastMCP Updates"
[4]: https://gofastmcp.com/clients/client-only-package "Client-Only Package"
[5]: https://gofastmcp.com/clients/fastmcp-remote "fastmcp-remote"
[6]: https://gofastmcp.com/servers/testing "Testing FastMCP"

# FastMCP Advanced — 2) First executable server, client, and test

## 2.0 Objective

The first executable should prove the complete FastMCP path, not only decorator syntax:

```text
Python declaration
  -> FastMCP component registry
  -> MCP schema
  -> transport or in-memory session
  -> Client initialization
  -> list/call/read/get operations
  -> typed result
```

The canonical minimal FastMCP server remains intentionally small: construct `FastMCP`, decorate a function, and call `mcp.run()` under a `__main__` guard. The important implementation lesson is that **constructing/decorating defines the server; `run()` binds it to a transport**. ([Quickstart][1]) ([Running][2])

---

## 2.1 Minimal STDIO server

`server.py`:

```python
from fastmcp import FastMCP

mcp = FastMCP(
    "Calculator",
    instructions="Use the arithmetic tools for deterministic calculations.",
    version="1.0.0",
)


@mcp.tool
def add(a: int, b: int) -> int:
    """Add two integers."""
    return a + b


if __name__ == "__main__":
    mcp.run()  # STDIO by default
```

Run it directly:

```bash
python server.py
```

The process now speaks MCP on stdin/stdout; it is not a normal interactive CLI. A desktop/agent host normally owns the subprocess and protocol stream. ([Running][2])

### Critical STDIO rule

Do not print arbitrary text to stdout from server startup code. STDOUT is the protocol channel. Use Python logging to stderr or MCP context logging from within requests.

---

## 2.2 Minimal Streamable HTTP server

```python
from fastmcp import FastMCP

mcp = FastMCP("Calculator")


@mcp.tool
def add(a: int, b: int) -> int:
    return a + b


if __name__ == "__main__":
    mcp.run(
        transport="http",
        host="127.0.0.1",
        port=8000,
    )
```

The default FastMCP HTTP endpoint is typically `/mcp`; connect a client to the full MCP endpoint, not merely the host root. Streamable HTTP is the normal network deployment choice; SSE is retained for legacy clients. ([Running][2])

```python
from fastmcp import Client

async with Client("http://127.0.0.1:8000/mcp") as client:
    print(await client.ping())
```

---

## 2.3 Minimal in-memory client — preferred first test

You do **not** need a subprocess or network socket to test a FastMCP server.

```python
import asyncio
from fastmcp import Client, FastMCP

mcp = FastMCP("Calculator")


@mcp.tool
def add(a: int, b: int) -> int:
    return a + b


async def main() -> None:
    async with Client(mcp) as client:
        tools = await client.list_tools()
        print([tool.name for tool in tools])

        result = await client.call_tool("add", {"a": 2, "b": 3})
        print(result.data)


if __name__ == "__main__":
    asyncio.run(main())
```

`Client(mcp)` uses an in-process transport while still exercising the MCP client/server boundary. This is the fastest deterministic validation path and the preferred foundation for unit/integration tests. ([Testing][3])

---

## 2.4 First server with all three core component types

```python
from fastmcp import FastMCP

mcp = FastMCP("ReferenceServer")


@mcp.tool
def multiply(a: float, b: float) -> float:
    """Multiply two numbers."""
    return a * b


@mcp.resource("config://app")
def app_config() -> str:
    """Current application configuration."""
    return '{"mode":"demo"}'


@mcp.prompt
def review(topic: str) -> str:
    """Create a review request for a topic."""
    return f"Review {topic} for correctness, risk, and missing assumptions."


if __name__ == "__main__":
    mcp.run()
```

Client verification:

```python
from fastmcp import Client

async with Client(mcp) as client:
    tools = await client.list_tools()
    resources = await client.list_resources()
    prompts = await client.list_prompts()

    tool_result = await client.call_tool("multiply", {"a": 4.0, "b": 5.0})
    resource_result = await client.read_resource("config://app")
    prompt_result = await client.get_prompt("review", {"topic": "release plan"})
```

This is a better smoke test than importing the functions directly because it validates the published MCP surface rather than only local Python behavior.

---

## 2.5 First async tool

```python
import httpx
from fastmcp import FastMCP

mcp = FastMCP("HTTP")


@mcp.tool(timeout=10.0)
async def fetch_status(url: str) -> int:
    async with httpx.AsyncClient(timeout=5.0) as client:
        response = await client.get(url)
        return response.status_code
```

Use `async def` for naturally async I/O. FastMCP can run ordinary synchronous tools in a threadpool, but an async-native library should normally stay async to avoid unnecessary thread dispatch. ([Tools][4])

---

## 2.6 First typed tool with Pydantic metadata

```python
from typing import Annotated, Literal

from fastmcp import FastMCP
from pydantic import Field

mcp = FastMCP("Typed")


@mcp.tool
def search(
    query: Annotated[
        str,
        Field(min_length=1, max_length=200, description="Search phrase"),
    ],
    limit: Annotated[
        int,
        Field(ge=1, le=100, description="Maximum number of results"),
    ] = 20,
    mode: Literal["exact", "fuzzy"] = "fuzzy",
) -> dict:
    return {"query": query, "limit": limit, "mode": mode, "items": []}
```

Before writing custom schema JSON, inspect what FastMCP already generates from the function signature.

---

## 2.7 First hidden runtime dependency

```python
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("DI")


def current_tenant() -> str:
    return "tenant-a"


@mcp.tool
async def list_orders(
    status: str,
    tenant: str = Depends(current_tenant),
) -> list[dict]:
    # `tenant` is runtime-only and does not appear in the tool schema.
    return []
```

This demonstrates the key schema boundary: **client parameters are explicit typed arguments; runtime infrastructure is injected**. ([DI][5])

---

## 2.8 First Context-aware tool

```python
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

mcp = FastMCP("Context")


@mcp.tool
async def process(
    item_id: str,
    ctx: Context = CurrentContext(),
) -> dict:
    await ctx.info(f"Processing {item_id}")
    await ctx.report_progress(0.5, 1.0, "Halfway")
    return {"item_id": item_id, "ok": True}
```

Do not pass `Context` through domain layers that do not need MCP capabilities; use it at the adapter edge. ([Context][6])

---

## 2.9 First lifespan-managed dependency

```python
from fastmcp import FastMCP
from fastmcp.server.lifespan import lifespan


@lifespan
async def server_lifespan(server):
    client = await create_expensive_client()
    try:
        yield {"client": client}
    finally:
        await client.aclose()


mcp = FastMCP("Lifecycle", lifespan=server_lifespan)
```

Tool:

```python
from fastmcp import Context


@mcp.tool
async def query_backend(q: str, ctx: Context) -> dict:
    client = ctx.lifespan_context["client"]
    return await client.query(q)
```

Use lifespan for pools, shared clients, caches, models, and other server-instance resources that should not be re-created per tool call. ([Lifespan][7])

---

## 2.10 First HTTP app export

For production ASGI hosting:

```python
from fastmcp import FastMCP

mcp = FastMCP("Analytics")


@mcp.tool
def health_value() -> int:
    return 1


app = mcp.http_app(path="/mcp")
```

Run with an ASGI server:

```bash
uvicorn server:app --host 0.0.0.0 --port 8000
```

When mounting `mcp_app` inside a parent FastAPI/Starlette app, the FastMCP lifespan must be forwarded or combined. This is a correctness requirement for the session manager and other lifecycle state. ([HTTP][8])

---

## 2.11 First FastAPI mount

```python
from fastapi import FastAPI
from fastmcp import FastMCP

mcp = FastMCP("Analytics")


@mcp.tool
def summarize(values: list[float]) -> dict:
    return {"count": len(values), "sum": sum(values)}


mcp_app = mcp.http_app(path="/mcp")
app = FastAPI(lifespan=mcp_app.lifespan)
app.mount("/analytics", mcp_app)
```

Externally reachable MCP path:

```text
/analytics/mcp
```

Do not accidentally combine a mount prefix and internal MCP path twice. ([FastAPI Integration][9])

---

## 2.12 First pytest fixture

`tests/conftest.py`:

```python
import pytest
from fastmcp import Client

from my_server.server import mcp


@pytest.fixture
async def client():
    async with Client(mcp) as client:
        yield client
```

`tests/test_tools.py`:

```python
async def test_add(client):
    result = await client.call_tool("add", {"a": 2, "b": 5})
    assert result.data == 7


async def test_tool_catalog(client):
    names = {tool.name for tool in await client.list_tools()}
    assert "add" in names
```

This catches schema/registration/transport behavior that a direct `add(2, 5)` unit test cannot. ([Testing][3])

---

## 2.13 First contract snapshot

```python
from inline_snapshot import snapshot


async def test_tool_schema(client):
    tools = await client.list_tools()
    add = next(t for t in tools if t.name == "add")

    assert add.inputSchema == snapshot(
        {
            "type": "object",
            "properties": {
                "a": {"type": "integer"},
                "b": {"type": "integer"},
            },
            "required": ["a", "b"],
        }
    )
```

Exact protocol objects may contain more metadata depending on version/config. Snapshot only fields that represent the contract you actually intend to freeze.

---

## 2.14 First `fastmcp.json`

A portable project can use a FastMCP configuration file instead of embedding every run option in shell commands.

Representative shape:

```json
{
  "$schema": "https://gofastmcp.com/public/schemas/fastmcp.json/latest.json",
  "source": "src/my_server/server.py:mcp"
}
```

Then run through FastMCP CLI:

```bash
fastmcp run fastmcp.json
```

Use the exact current schema documented for your stable version; treat the example as the minimal conceptual shape and see §27 for the complete configuration contract. ([Configuration][10])

---

## 2.15 First CLI inspection loop

Recommended development loop:

```bash
# show installed versions
fastmcp version

# inspect local server object
fastmcp inspect src/my_server/server.py:mcp

# run it
fastmcp run src/my_server/server.py:mcp

# from another shell, inspect a remote endpoint
fastmcp list http://127.0.0.1:8000/mcp
```

Depending on the local FastMCP CLI version, the exact command surface may include additional `dev`, Inspector, discovery, or host-install flows. Use `fastmcp --help` and §28 rather than guessing flags.

---

## 2.16 Synchronous vs asynchronous application boundaries

`FastMCP.run()` is synchronous and owns its event loop. Do not call it from inside an already-running async function.

Correct synchronous launcher:

```python
if __name__ == "__main__":
    mcp.run(transport="http")
```

Correct async launcher:

```python
import asyncio


async def main():
    await mcp.run_async(transport="http")


if __name__ == "__main__":
    asyncio.run(main())
```

For web frameworks, prefer `http_app()` so the outer ASGI runtime owns process/event-loop lifecycle. ([Running][2])

---

## 2.17 What direct Python calls do not test

Calling a decorated function directly is useful for domain correctness:

```python
assert add(2, 3) == 5
```

But it does **not** prove:

* the component is registered;
* its public name is what you expect;
* its JSON Schema is correct;
* hidden dependencies are omitted;
* middleware runs;
* authorization is enforced;
* return conversion is correct;
* version/visibility selection is correct;
* the transport/handshake works.

Use both ordinary unit tests and MCP-boundary tests.

---

## 2.18 First production-shaped separation

`domain.py`:

```python
class OrderService:
    async def get_order(self, order_id: str, tenant_id: str) -> dict:
        ...
```

`dependencies.py`:

```python
from fastmcp.dependencies import Depends


async def get_order_service() -> OrderService:
    ...
```

`tools/orders.py`:

```python
from fastmcp.dependencies import Depends


@mcp.tool
async def get_order(
    order_id: str,
    service: OrderService = Depends(get_order_service),
) -> dict:
    return await service.get_order(order_id, tenant_id="...")
```

The key architecture is the separation: the MCP function is a thin adapter over a separately testable service, while runtime infrastructure is injected rather than exposed as model-visible arguments.

---

## 2.19 Failure-mode checklist for the first deployment

| Symptom | Likely cause |
|---|---|
| Host reports server immediately exited | import error, wrong interpreter, missing dependency, or startup exception |
| Host “connects” but no tools | wrong server object/source, visibility transform, proxy upstream issue, or registration code not imported |
| HTTP client gets 404 | connected to host root instead of MCP path, or mount/path composition wrong |
| Tool input rejected | schema/type mismatch, strict validation, incorrect JSON shape |
| Tool works direct but fails over MCP | hidden dependency/context/result-conversion/auth issue |
| Mounted HTTP server hangs/fails sessions | FastMCP lifespan not propagated |
| STDIO protocol corruption | application printed arbitrary text to stdout |
| Remote bridge initializes empty | stale proxy behavior or wrong backend; 3.4 bridge should fail loudly on initialization errors |
| `Client(...)` call before context entry fails | session not initialized / lifecycle not entered |

---

## 2.20 First-app agent invariants

1. `FastMCP(...)` defines; `run*()` serves.
2. `@mcp.tool`/resource/prompt registration occurs at import/declaration time.
3. `Client` operations are async and normally live inside `async with`.
4. In-memory `Client(mcp)` is the default test transport.
5. Streamable HTTP is the default new network transport; SSE is legacy.
6. Keep `mcp.run()` under `if __name__ == "__main__":`.
7. Keep stdout clean for STDIO.
8. Use `http_app()` when another ASGI framework owns serving.
9. Forward/combine FastMCP lifespan when mounting.
10. Test the published MCP surface, not only the underlying Python functions.

---

## 2.21 Anti-pattern inventory

* `mcp.run()` at module import time.
* calling `mcp.run()` inside an async FastAPI startup hook.
* testing only direct Python function calls.
* using a real TCP port for every unit test when in-memory Client works.
* putting runtime-only DB handles into public tool parameters.
* printing banners/debug text to stdout under STDIO.
* connecting `Client("http://host:8000")` when the endpoint is `/mcp`.
* mounting `/mcp` and also using internal `path="/mcp"` without checking final path.
* creating a new expensive HTTP/DB client on every tool call instead of lifespan/DI.
* using `collect everything into one server.py` as the project grows.

---

## 2.22 First executable checklist

```text
[ ] Server object imports successfully.
[ ] mcp.run() is protected by __main__.
[ ] STDIO startup emits no arbitrary stdout.
[ ] In-memory Client can initialize.
[ ] list_tools/resources/prompts matches expected contract.
[ ] At least one tool is invoked through Client.
[ ] Typed validation failure is tested.
[ ] Lifespan startup/cleanup is tested if shared state exists.
[ ] HTTP endpoint path is verified if using network transport.
[ ] Mounted ASGI lifespan is forwarded.
[ ] Production code separates domain logic from MCP adapters.
```

[1]: https://gofastmcp.com/getting-started/quickstart "FastMCP Quickstart"
[2]: https://gofastmcp.com/deployment/running-server "Running FastMCP"
[3]: https://gofastmcp.com/servers/testing "Testing FastMCP"
[4]: https://gofastmcp.com/servers/tools "Tools"
[5]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection"
[6]: https://gofastmcp.com/servers/context "Context"
[7]: https://gofastmcp.com/servers/lifespan "Lifespans"
[8]: https://gofastmcp.com/deployment/http "HTTP Deployment"
[9]: https://gofastmcp.com/integrations/fastapi "FastAPI Integration"
[10]: https://gofastmcp.com/deployment/server-configuration "Project Configuration"

# FastMCP Advanced — 3) Core API map and object model
### Core API map and object model

### Version frame

FastMCP’s current object model is materially shaped by the v3 architecture shift: providers and transforms became first-class primitives in `3.0.0`, and `FastMCP()` stopped owning transport/host/port configuration in v3. In other words: treat the server constructor as **identity + composition + behavior + handlers/storage**, and treat actual serving as a later runtime step (`run()`, `http_app()`, CLI, etc.), not constructor state. ([FastMCP][1])

### 3.0 Canonical ownership map

The shortest correct “what owns what” model is:

* **`FastMCP`** owns server identity, local component registration, provider aggregation, server-level transforms, middleware, auth, lifecycle, visibility filtering, session-aware filtering, and public execution APIs like `call_tool`, `read_resource`, and `render_prompt`. ([FastMCP][2])
* **`LocalProvider`** owns components declared directly in Python via decorators or programmatic registration; `@mcp.tool`, `@mcp.resource`, `@mcp.prompt`, and `mcp.add_tool()` ultimately populate this provider. ([FastMCP][3])
* **`Provider`** is the abstract component-source boundary: it lists/gets tools/resources/prompts/templates/tasks, can carry provider-level transforms, and can be wrapped immutably for reuse. It is the correct abstraction for modular subservers, proxies, filesystem/OpenAPI-backed sources, and custom dynamic catalogs. ([FastMCP][4])
* **`Transform`** is not a component store; it is a component-rewrite layer between providers and client-visible output. Provider transforms apply to one source; server transforms apply after aggregation to everything. ([FastMCP][5])
* **`Client`** owns transport/session/protocol interaction from the consumer side. It does not own server declarations or schemas; it consumes what the server publishes. ([FastMCP][6])
* **`FastMCPApp`** is itself a **Provider** specialized for interactive UI systems: model-visible entry tools, app-visible backend tools, and the renderer resource live together there, then get attached to a server through the normal provider composition path. ([FastMCP][7])

That model is the stable base for all later sections. If an agent confuses “server container” with “provider source” or “app provider” with “UI transport,” everything downstream becomes muddy. ([FastMCP][3])

---

## 3.1 `FastMCP(...)` — the server container / aggregate execution surface

### 3.1.1 Server role

`FastMCP` is the central server object. The public docs define it as the container for tools, resources, and prompts, and the SDK surface shows that it also owns middleware registration, provider aggregation, server transforms, public execution APIs, lifecycle entrypoints, and import/mount/proxy helpers. ([FastMCP][2])

### 3.1.2 Constructor mental model

The constructor surface is intentionally grouped by responsibility:

* **Identity**: `name`, `instructions`, `version`, `website_url`, `icons`
* **Composition**: `tools`, `auth`, `middleware`, `providers`, `transforms`, `lifespan`
* **Behavior**: `on_duplicate`, `strict_input_validation`, `mask_error_details`, `list_page_size`, `tasks`, `client_log_level`, `dereference_schemas`
* **Handlers / storage**: `sampling_handler`, `sampling_handler_behavior`, `session_state_store` ([FastMCP][2])

A syntactically representative construction pattern is:

```python
from fastmcp import FastMCP

mcp = FastMCP(
    "Platform",
    instructions="Use the structured tools first; prefer search over broad listing when catalogs are large.",
    version="1.2.0",
    providers=[],
    transforms=[],
    middleware=[],
    strict_input_validation=True,
    tasks=True,
)
```

That is the right mental model for the constructor: describe **what the server is built from** and **how it should behave**. Do **not** put transport runtime there in v3. Transport belongs at serve time. ([FastMCP][2])

### 3.1.3 `FastMCP` owns a `local_provider`

This is the most important hidden object-model fact. `FastMCP.local_provider` is the storage backing for directly registered components, and the provider overview states explicitly that `@mcp.tool` writes into the server’s `LocalProvider`. The SDK page also exposes `local_provider(self) -> LocalProvider` and recommends using it for component removal. ([FastMCP][8])

Implication: decorator-defined components are **not** a magical separate registry. They are just the first provider in the aggregate server. That is why local components always win precedence over added providers. ([FastMCP][8])

### 3.1.4 Registration and public execution APIs

`FastMCP` owns both declaration APIs and execution APIs:

* Declaration: `tool(...)`, `add_tool(...)`, `resource(...)`, `add_resource(...)`, `prompt(...)`, `add_prompt(...)`
* Resolution/listing: `list_tools()`, `get_tool(...)`, `list_resources()`, `get_resource(...)`, `list_prompts()`, `get_prompt(...)`
* Execution/rendering: `call_tool(...)`, `read_resource(...)`, `render_prompt(...)` ([FastMCP][8])

This matters architecturally: `FastMCP` is not only a registration container; it is also the server-side **resolved execution facade** after aggregation, auth filtering, middleware, visibility rules, and task handling are applied. ([FastMCP][8])

### 3.1.5 Final filtering happens at the server, not the provider

`Provider.list_tools()` and `Provider.get_tool()` intentionally do **not** do final enabled/disabled filtering. Provider methods return transformed components, possibly marked disabled; FastMCP then performs visibility/auth/session-aware filtering after all transforms complete. The SDK explicitly says this design allows session transforms to override provider-level disables. ([FastMCP][4])

This is a crucial ownership boundary:

* **Provider** owns sourcing + provider-local transform application.
* **Server** owns final client-visible filtering and runtime policy. ([FastMCP][4])

### 3.1.6 Provider aggregation order and precedence

`FastMCP.add_provider(...)` adds a dynamic provider source. Providers are queried in registration order, the first non-`None` result wins, and static components registered via decorators always take precedence. The provider overview says `LocalProvider` is always first; additional providers follow in the order added. ([FastMCP][8])

Agent rule: if two providers can publish the same tool name, resolution order is a semantic part of your API surface. Namespacing or transforms are safer than relying on “first wins” collision behavior. ([FastMCP][3])

---

## 3.2 `Provider` — the component-source abstraction

### 3.2.1 Base role

`Provider` is the base class for dynamic component providers. Default implementations return empty results, so subclasses only implement what they support. The provider overview defines a provider as a source of tools, resources, and prompts; when a client asks what exists, FastMCP asks providers and combines results; when a client calls a component, FastMCP delegates to the provider that owns it. ([FastMCP][4])

### 3.2.2 Core provider methods

The base SDK surface includes:

* `list_tools() -> Sequence[Tool]`
* `get_tool(name, version=None) -> Tool | None`
* `get_app_tool(app_name, tool_name) -> Tool | None`
* analogous resource/template/prompt methods
* `get_tasks()`
* `lifespan()`
* visibility helpers like `enable(...)` / `disable(...)`
* transform methods `add_transform(...)` and `wrap_transform(...)` ([FastMCP][4])

This means a Provider is not just a “list container”; it participates in lookup, lifecycle, task surfacing, transform composition, and visibility defaults. ([FastMCP][4])

### 3.2.3 Built-in provider categories

The provider overview identifies the built-in source categories most relevant to object modeling:

* **`LocalProvider`** — components defined in code
* **`FastMCPProvider`** — wraps another FastMCP server, usually via `mount(server)`
* **`ProxyProvider`** — exposes a remote MCP server locally
* plus custom/dynamic sources for databases, OpenAPI specs, and other generated catalogs ([FastMCP][3])

This is the right abstraction boundary for modularity. If a component source can vary independently from the root server, it should probably be a Provider. ([FastMCP][3])

### 3.2.4 Mutating vs immutable provider transforms

`Provider.add_transform(transform)` mutates the provider. `Provider.wrap_transform(transform)` returns a new wrapped provider without changing the original. The SDK explicitly calls out `wrap_transform()` as the right choice when the same provider is reused in multiple aggregators with different namespaces or transform stacks. ([FastMCP][4])

Agent rule: default to `wrap_transform()` when building reusable provider libraries; use `add_transform()` when the provider instance is intentionally single-owner and mutable. ([FastMCP][4])

---

## 3.3 `Transform` — the client-visible rewrite layer

### 3.3.1 Role

Transforms modify components as they flow from providers to clients. The docs describe them as filters in a pipeline: `Provider → [Transform A] → [Transform B] → Client`. When listing, transforms act like pure sequence transforms; when resolving a single named component, they behave like middleware, mapping the client-visible name back to the original and then transforming the result on the way out. ([FastMCP][5])

### 3.3.2 Placement levels

Transforms can exist at two levels:

* **Provider-level transforms**: only affect one source; run first.
* **Server-level transforms**: affect the aggregated output of all providers; run after provider transforms. Later transforms override earlier ones, so the server has final say. ([FastMCP][5])

This placement distinction is the correct answer to “where should I namespace, filter, or rewrite?”:

* source-specific rewrite → provider transform
* global policy / final exposure rewrite → server transform ([FastMCP][5])

### 3.3.3 Common built-ins

The transforms overview lists the built-in families most relevant to the object model:

* `Namespace`
* `ToolTransform`
* `Enabled`
* `Tool Search`
* `Resources as Tools`
* `Prompts as Tools`
* experimental `Code Mode` ([FastMCP][5])

The value case is direct: transforms let you change **what clients see** without changing the underlying provider or local declarations. That is the central decoupling primitive of v3. ([FastMCP][5])

### 3.3.4 Namespacing is transform-driven

`Namespace("api")` prefixes tools/prompts with `api_...` and resources/templates with URI path prefixes. The providers overview says `mount(server, namespace="api")` creates this transform automatically. ([FastMCP][9])

```python
from fastmcp import FastMCP

weather = FastMCP("Weather")
calendar = FastMCP("Calendar")

main = FastMCP("Main")
main.mount(weather, namespace="weather")
main.mount(calendar, namespace="calendar")
```

This is the canonical composition-safe syntax for avoiding collisions between mounted servers. ([FastMCP][9])

---

## 3.4 `FastMCPApp(...)` — app provider / UI-capability bundle

### 3.4.1 Object role

`FastMCPApp` is a **Provider** representing a composable MCP application. It bundles entry-point tools (`@app.ui()`), backend tools (`@app.tool()`), and the Prefab renderer resource. The SDK and app docs both say backend tools are tagged in `meta["fastmcp"]["app"]` so they remain discoverable through provider composition even when transforms rename or hide them. ([FastMCP][7])

### 3.4.2 Entry tools vs backend tools

`@app.ui()` registers model-visible entry points. These default to `visibility=["model"]`, appear in the tool list, and return Prefab UI. They support tool-like options such as `name`, `description`, `title`, `tags`, `icons`, `auth`, and `timeout`. ([FastMCP][10])

`@app.tool()` registers backend tools callable by the UI via `CallTool`. These default to `visibility=["app"]`; passing `model=True` exposes them to both UI and model. They support options such as `name`, `description`, `auth`, and `timeout`. ([FastMCP][7])

### 3.4.3 Why `FastMCPApp` exists instead of “just use `app=True` everywhere”

The interactive-app docs give the exact problem statement: once a UI has multiple backend tools, you need stable backend identifiers, automatic visibility partitioning, and namespace-safe routing. `FastMCPApp` solves those by making the app a provider and keeping UI-facing backend lookup separate from the normal transformed model-visible lookup. ([FastMCP][10])

### 3.4.4 `get_app_tool` bypass

Normal tool resolution goes through the provider chain and all transforms. App UI tool calls do not. When the server sees `_meta.fastmcp.app` on a UI `CallTool` request, it calls `get_app_tool(app_name, tool_name)` instead of `get_tool(name)`. The provider base defines `get_app_tool(...)` specifically as an original-name lookup that bypasses transforms. This is how `save_contact` still works even if a namespace transform renamed the client-visible tool to `contacts_save_contact`. Authorization still applies. ([FastMCP][11])

This is one of the most important ownership boundaries in the whole framework:

* model-visible resolution = transformed provider chain
* app-backend resolution = provider-tree bypass keyed by original tool name + app identity ([FastMCP][11])

### 3.4.5 Attachment syntax

`FastMCPApp` is attached like any other provider:

```python
from fastmcp import FastMCP, FastMCPApp

app = FastMCPApp("Notes")

@app.ui()
def notes_app() -> "PrefabApp":
    ...

@app.tool()
def add_note(title: str, body: str) -> list[dict]:
    ...

mcp = FastMCP("Notes Server", providers=[app])
# or:
# mcp.add_provider(app)
```

That is the cleanest “app is provider” syntax. ([FastMCP][7])

### 3.4.6 Other app providers are still providers

The apps overview shows that `GenerativeUI()` is added with `mcp.add_provider(GenerativeUI())`, and the SDK pages for Approval, Choice, FormInput, FileUpload, and GenerativeUI all describe them as Providers. That means “Apps” are not a second server subsystem; they compose through the same provider mechanism as everything else. ([FastMCP][12])

---

## 3.5 `Client(...)` — the programmatic consumer

### 3.5.1 Role

`Client` is the explicit, deterministic MCP consumer. The docs are precise: it is designed for controlled interactions rather than autonomous behavior, and it delegates connection management to a transport while owning MCP protocol logic. ([FastMCP][6])

### 3.5.2 Construction surface

The SDK docs say the first constructor argument is a transport/source specification and can be:

* `ClientTransport`
* in-process `FastMCP`
* `AnyUrl | str`
* `Path`
* `MCPConfig`
* `dict` transport config

Optional constructor concerns include `roots`, sampling/log/message/progress handlers, request `timeout`, and `init_timeout`. ([FastMCP][13])

Representative forms:

```python
from fastmcp import Client, FastMCP

server = FastMCP("TestServer")
client_a = Client(server)                     # in-process
client_b = Client("https://example.com/mcp") # remote URL
client_c = Client("my_server.py")            # local server spec / script path
```

These are the documented source categories for transport inference. ([FastMCP][6])

### 3.5.3 Session ownership and lifecycle

`Client` owns session state, not server state. The SDK surfaces `session`, `initialize_result`, `is_connected()`, `initialize()`, `close()`, and `new()`. `initialize()` performs the MCP handshake and is idempotent; entering `async with client:` initializes automatically unless `auto_initialize=False` was used. `new()` clones configuration but gives you a fresh, disconnected session state. ([FastMCP][13])

This is the correct pattern for agents:

* reuse one client instance for nested use when shared session state is desirable
* call `client.new()` when you want an independent session with the same transport/configuration ([FastMCP][13])

### 3.5.4 Reentrant contexts are supported

The client SDK docs explicitly call out reentrant context-manager support via reference counting and background session management, motivated by race conditions exposed by automatic `list_tools()` behavior in MCP SDK 1.10. That means nested or concurrent `async with client:` usage is a deliberate supported behavior, not an accident. ([FastMCP][13])

### 3.5.5 Client/server boundary

`Client` does not own declarations, transforms, or provider composition. It receives the already-published client-visible view. Its job is: initialize, inspect capabilities, invoke tools/resources/prompts, and optionally participate in advanced flows like roots, sampling, elicitation, progress, and logging. ([FastMCP][6])

---

## 3.6 Composition methods: mount, proxy, import

### 3.6.1 `mount(...)` — dynamic nested-server composition

`FastMCP.mount(server, namespace=None, ...)` mounts another FastMCP server dynamically. The SDK distinguishes mounting from importing: mounted requests are forwarded in real time, so changes to the mounted server are reflected immediately when accessed through the parent. The providers overview says mounted servers become `FastMCPProvider`s. ([FastMCP][8])

Agent rule: prefer `mount()` for live composition. It preserves middleware/lifespan behavior and keeps the child server an actual component source rather than a copied snapshot. ([FastMCP][8])

### 3.6.2 `import_server(...)` — one-time copy, deprecated

`import_server(...)` is deprecated in favor of `mount()`. The SDK says it performs a one-time import of the other server’s objects, future changes are not reflected, and server-level configurations/lifespans are not imported. ([FastMCP][8])

Agent rule: only use `import_server()` for legacy code or when you intentionally want copy semantics. For modular systems, it is the wrong primitive. ([FastMCP][8])

### 3.6.3 `create_proxy(...)` / `FastMCPProxy` — remote-to-local server projection

`create_proxy(target, **settings)` is the recommended way to build a proxy server for a remote or otherwise external backend. `FastMCPProxy` is described as a convenience server around `ProxyProvider`, and `ProxyClient` exists to forward advanced interactions like roots, sampling, elicitation, logging, and progress. `FastMCP.as_proxy(...)` exists but is deprecated in favor of `create_proxy(...)`. ([FastMCP][8])

---

## 3.7 Deployment and design advisories tied to the object model

**Advisory 1 — keep transport out of the constructor.** In v3, `FastMCP()` is not the deployment object; it is the server-definition object. Bind transport when serving. ([FastMCP][14])

**Advisory 2 — think in provider boundaries early.** Even if a first version is a single file of decorators, the docs’ architecture makes clear that every server already has at least one provider. If you expect composition, reuse, namespacing, or remote sources, model them as providers immediately. ([FastMCP][3])

**Advisory 3 — use provider transforms for source-local policy; server transforms for final exposure policy.** This preserves reuse and lets the root server own the final client-visible contract. ([FastMCP][5])

**Advisory 4 — use `wrap_transform()` when sharing providers.** Mutable provider transforms are fine for single ownership; reusable provider libraries should prefer immutable wrapping to avoid accidental cross-aggregator mutation. ([FastMCP][4])

**Advisory 5 — use `FastMCPApp` once a UI has real backend topology.** Single-display `app=True` tools are fine; multi-tool UIs with namespacing, visibility separation, and refactor-safe backend references should become app providers. ([FastMCP][10])

**Advisory 6 — resolve collisions intentionally.** Local provider first, then providers in registration order, first hit wins. Relying on accidental order is brittle; use namespaces or explicit transforms. ([FastMCP][8])


[1]: https://gofastmcp.com/updates "FastMCP Updates - FastMCP"
[2]: https://gofastmcp.com/servers/server "The FastMCP Server - FastMCP"
[3]: https://gofastmcp.com/servers/providers/overview "Providers - FastMCP"
[4]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-base "base - FastMCP"
[5]: https://gofastmcp.com/servers/transforms/transforms "Transforms Overview - FastMCP"
[6]: https://gofastmcp.com/clients/client "The FastMCP Client - FastMCP"
[7]: https://gofastmcp.com/python-sdk/fastmcp-apps-app "app - FastMCP"
[8]: https://gofastmcp.com/python-sdk/fastmcp-server-server "server - FastMCP"
[9]: https://gofastmcp.com/servers/transforms/namespace "Namespace Transform - FastMCP"
[10]: https://gofastmcp.com/apps/interactive-apps "FastMCPApp - FastMCP"
[11]: https://gofastmcp.com/apps/architecture "App Architecture - FastMCP"
[12]: https://gofastmcp.com/apps/overview "Apps - FastMCP"
[13]: https://gofastmcp.com/python-sdk/fastmcp-client-client "client - FastMCP"
[14]: https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2 "Upgrading from FastMCP 2 - FastMCP"

# FastMCP Advanced — 4) Server construction and lifecycle
### Server construction and lifecycle

### Version anchors

For `FastMCP(...)`, the decisive architectural break is v3: transport-facing kwargs such as `host`, `port`, `streamable_http_path`, old include/exclude tag kwargs, and mutable tool-transformation constructor kwargs were removed from the constructor and are now expected at serve time (`run_http_async()`, `http_app()`, environment/settings) or as post-construction visibility/transform calls. The docs also frame the constructor around identity, composition, behavior, and handlers/storage, rather than transport binding. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### 4.0 Constructor surface: exact mental model

The current main-branch source defines `FastMCP.__init__` with the following surface. This is the best “call-signature-first” anchor for the rest of the section. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

```python id="064zod"
FastMCP(
    name: str | None = None,
    instructions: str | None = None,
    *,
    version: str | int | float | None = None,
    website_url: str | None = None,
    icons: list[mcp.types.Icon] | None = None,
    auth: AuthProvider | None = None,
    middleware: Sequence[Middleware] | None = None,
    providers: Sequence[Provider] | None = None,
    transforms: Sequence[Transform] | None = None,
    lifespan: LifespanCallable | Lifespan | None = None,
    tools: Sequence[Tool | Callable[..., Any]] | None = None,
    on_duplicate: Literal["warn", "error", "replace", "ignore"] | None = None,
    mask_error_details: bool | None = None,
    dereference_schemas: bool = True,
    strict_input_validation: bool | None = None,
    list_page_size: int | None = None,
    tasks: bool | None = None,
    session_state_store: AsyncKeyValue | None = None,
    sampling_handler: SamplingHandler | None = None,
    sampling_handler_behavior: Literal["always", "fallback"] | None = None,
    client_log_level: mcp.types.LoggingLevel | None = None,
    **kwargs: Any,
)
```

This is not “just metadata plus decorators.” The constructor establishes the server’s identity envelope, local component registry, provider aggregation order, transform stack, middleware chain, low-level MCP server wrapper, state store, task defaults, schema-processing policy, and lifecycle entry behavior. By the time `__init__` returns, the object is already the authoritative server-definition object; later `run()` / `http_app()` only decide how that definition is exposed over transport. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py), [gofastmcp.com](https://gofastmcp.com/servers/server))

---

## 4.1 Minimal construction and the “always set these” rule

The documented minimum is one argument: a server name. Instructions are the next-highest-value field because clients surface them to guide LLM/tool selection behavior. ([gofastmcp.com](https://gofastmcp.com/servers/server))

```python id="2zct6i"
from fastmcp import FastMCP

mcp = FastMCP(
    "DataAnalysis",
    instructions="Provides tools for analyzing numerical datasets. Start with get_summary() for an overview.",
)
```

There is one subtlety worth documenting explicitly for agents: the prose docs describe `name` as defaulting to `"FastMCP"`, but the current main-branch source accepts `name: str | None = None` and passes `name or self.generate_name()` into the low-level server. The safest documentation rule is therefore: **always set `name` explicitly**; do not rely on implicit naming behavior in generated docs, tests, or deployment manifests. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

---

## 4.2 Identity fields

### `name`

`name` is the human-readable server identity presented to clients and logs. Operationally, it is also fed into the underlying `LowLevelServer`, so it becomes part of the wire-visible server identity, not just a local Python label. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `instructions`

`instructions` are surfaced to clients to help LLMs understand server purpose and usage patterns. In the SDK surface, `instructions` is not frozen constructor-only state; it is exposed as a property with a setter, so post-construction mutation is possible. Use that sparingly: mutating instructions after clients have already initialized can create state drift between live behavior and previously cached client/server metadata. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `version`

The server docs say `version` defaults to the FastMCP library version if omitted. The source is more precise: constructor input accepts `str | int | float | None`, is coerced via `_coerce_version`, and if still unset the low-level server receives `fastmcp.__version__`. Agent rule: always set a semantic server version explicitly if the server is a product/API surface; otherwise client-visible version will track library upgrades rather than your server release cadence. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `website_url` and `icons`

These fields are presentational but protocol-relevant: they are forwarded into the low-level server object and exposed to clients as server metadata. They belong in the constructor, not in ad hoc resource/tool metadata, when the information describes the server as a whole. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

---

## 4.3 Composition fields: what the server is built from

### `tools`

`tools` is the constructor-time alternative to `@mcp.tool`. The high-level docs describe it as a programmatic registration path; the source shows the exact coercion rule: if an item is not already a `Tool`, FastMCP converts it with `Tool.from_function(...)` and then registers it via `add_tool(...)`. This is the correct constructor argument when you are assembling servers from prebuilt callables or registries rather than defining components inline. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

```python id="mon5jq"
from fastmcp import FastMCP

def ping() -> str:
    return "pong"

mcp = FastMCP(
    "ProgrammaticServer",
    tools=[ping],   # coerced to Tool internally
)
```

### `auth`

The server docs scope `auth` to HTTP-based transports. The source makes the transport distinction explicit: auth checks are skipped on STDIO transport and only meaningful on HTTP-based request flows. Deployment rule: do not treat constructor `auth=` as a universal security boundary; it is an HTTP boundary. STDIO security remains process-level / OS-level / launcher-level. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `middleware`

`middleware` is a constructor-time way to install cross-cutting behavior over all MCP operations. FastMCP middleware is a framework feature, not a protocol feature, and the execution model is bidirectional pipeline semantics: request flows inward in declaration order, response flows outward in reverse order. Middleware order is semantically significant; error handling belongs early, logging often belongs late. When servers are mounted, parent middleware runs for all child requests, and child middleware runs only for the child-handled operations. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/servers/middleware))

### `providers`

`providers` attach dynamic component sources. The server docs frame them as request-time sources that can serve tools/resources/prompts from APIs, databases, or external systems. The constructor does not inline-copy their contents; it stores them as part of the aggregate provider graph. Source-level initialization confirms that all constructor-supplied providers are added after the local provider, in order. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `transforms`

`transforms` are server-level component rewrites applied after provider aggregation. This is the right place for final client-visible shaping: namespacing, search-collapse, enable/disable overlays, tool/resource/prompt adaptation, and other publication-time contract rewrites. Constructor-supplied transforms are added in order during initialization; later transforms override earlier ones. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/servers/transforms/transforms), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `lifespan`

`lifespan` is the server-level startup/teardown hook. In v3 it is a first-class composable construct, runs exactly once per server start rather than once per session, and yields a context dict accessible from tools via `ctx.lifespan_context`. The source shows FastMCP storing the provided lifespan callable or falling back to `default_lifespan`, then wrapping it into the low-level server via a proxy. Deployment rule: expensive shared state belongs here, not in ad hoc globals or per-request lazy singletons. Always use `try/finally` for cleanup. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/servers/lifespan), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

```python id="ul4g4k"
from fastmcp import FastMCP, Context
from fastmcp.server.lifespan import lifespan

@lifespan
async def app_lifespan(server):
    db = await connect_db()
    try:
        yield {"db": db}
    finally:
        await db.close()

mcp = FastMCP("MyServer", lifespan=app_lifespan)

@mcp.tool
async def list_users(ctx: Context) -> list[str]:
    db = ctx.lifespan_context["db"]
    return await db.fetch_users()
```

---

## 4.4 Behavior fields: request-processing policy

### `on_duplicate`

`on_duplicate` accepts `"warn"`, `"error"`, `"replace"`, or `"ignore"`; the documented default is `"warn"`. This governs duplicate component registration behavior across local registration and provider-fed addition patterns. Agent rule: for production servers, prefer `"error"` during development/test and only loosen it intentionally when replacing/versioning behavior is expected. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `strict_input_validation`

This toggles the input-validation contract. With the default flexible mode, Pydantic-style coercions are allowed, so `"10"` can satisfy an `int` parameter. With `strict_input_validation=True`, FastMCP validates against the exact generated JSON Schema before calling your function. Value case: flexible mode maximizes client compatibility; strict mode maximizes protocol correctness and catches “stringified number / bool / enum” bugs at the boundary. ([gofastmcp.com](https://gofastmcp.com/servers/server))

### `mask_error_details`

When enabled, internal error details are replaced with generic messages in tool/resource responses. The docs say the default can come from `FASTMCP_MASK_ERROR_DETAILS`; the source stores the final effective value at construction time. Production rule: turn this on for Internet-exposed servers unless you have a compelling debugging reason not to. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `list_page_size`

This limits `tools/list`, `resources/list`, and related list operations. `None` means unpaginated. The source additionally enforces `list_page_size > 0` and raises `ValueError` otherwise. Value case: large provider graphs, generated OpenAPI servers, and mounted/proxied catalogs benefit materially from pagination rather than forcing every client initialization to ingest the full registry. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `tasks`

This is the constructor-level default for background task support. The docs describe it as enabling tools/resources to return `CreateTaskResult`; the source stores it as `_support_tasks_by_default`. Deployment rule: do not switch this on by habit; enable it only when the server actually exposes task-capable operations or needs async workflow semantics. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `client_log_level`

This is the default minimum log level for messages emitted toward MCP clients via `context.log()`. Clients can override it per session using `logging/setLevel`. Value case: it is the server-side backpressure knob for log verbosity over the MCP channel, not the same thing as Python logger configuration. ([gofastmcp.com](https://gofastmcp.com/servers/server))

### `dereference_schemas`

The docs recommend leaving this enabled because many clients require flat schemas rather than `$ref` graphs. The source shows a stronger implementation detail: if `dereference_schemas=True`, FastMCP automatically appends `DereferenceRefsMiddleware()` to the middleware list during construction. That means this flag is not only “metadata generation policy”; it is also “auto-install schema-rewrite middleware.” ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

---

## 4.5 Handlers and storage

### `sampling_handler` and `sampling_handler_behavior`

These configure server-initiated sampling requests. The docs define `sampling_handler` as the custom handler and `sampling_handler_behavior` as `"fallback"` or `"always"`. Operationally: `"fallback"` preserves tool-specific handlers if present; `"always"` centralizes all sampling in the server-level handler. Choose `"always"` only if you need global mediation or unified routing. ([gofastmcp.com](https://gofastmcp.com/servers/server))

### `session_state_store`

The docs say this is a persistent key-value store for session state, defaulting to in-memory. The source shows the concrete implementation detail: if omitted, FastMCP uses `MemoryStore()`, wraps it in a `PydanticAdapter[StateValue]`, and writes under collection `"fastmcp_state"`. Deployment rule: in multi-process, restart-sensitive, or horizontally scaled HTTP deployments, do not rely on the default in-memory store; provide a real `AsyncKeyValue` backend. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

---

## 4.6 Constructor algorithm: what actually happens during `FastMCP(...)`

The constructor is more than field assignment. The current main-branch source executes a clear initialization sequence. First, `_check_removed_kwargs(kwargs)` rejects v2-era constructor kwargs and emits migration-guiding errors for removed options such as transport binding, include/exclude tags, and constructor-time tool transformations. Then `super().__init__()` initializes provider internals, including transform storage inherited from the provider/aggregate base. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

Next, FastMCP resolves task defaults, initializes internal docket/worker references, creates `_additional_http_routes`, creates the session state store, creates `LocalProvider(on_duplicate=...)`, and immediately adds that local provider first to the provider chain. Only after that does it attach constructor-supplied providers and constructor-supplied transforms in order. This is the point where the server becomes the aggregate component registry. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

Then constructor-level behavior policies are frozen into internal state: `mask_error_details`, `list_page_size`, and lifespan selection. If no lifespan is supplied, `default_lifespan` is used. The low-level MCP server is then created with the resolved `name`, `version`, `instructions`, `website_url`, `icons`, and a lifespan proxy. This is the moment identity metadata stops being “mere Python attributes” and becomes the underlying MCP server identity. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

After the low-level server exists, constructor-supplied tools are coerced and registered, `strict_input_validation` and `client_log_level` are resolved, constructor middleware is copied into `self.middleware`, optional dereference middleware is auto-appended, and `_setup_handlers()` wires protocol handlers into the low-level server. Finally, sampling-handler state is stored. That sequence is why the object is fully publishable immediately after construction. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

---

## 4.7 How the server becomes the authoritative registry

The decisive mechanism is `local_provider` + aggregate-provider inheritance. Direct registrations live in `local_provider`; additional providers are layered behind it; server transforms are layered above aggregation; server visibility/auth/middleware are layered above that. The SDK page exposes `local_provider`, `add_provider`, `add_transform`, and the resolved public listing/execution methods. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

Server-side listing methods such as `list_tools()`, `list_resources()`, and `list_prompts()` are not raw provider passthroughs. The SDK says they add visibility filtering, auth filtering, and middleware execution, and they return all versions without deduplication at this layer; protocol handlers deduplicate for MCP wire format later. Likewise, `get_tool()` / `get_resource()` / `get_prompt()` apply final enabled/disabled filtering after all transforms, including session-level ones, and can fall back from a disabled highest version to the next-highest enabled version when version was not requested explicitly. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

The execution APIs are also server-owned: `call_tool(...)`, `read_resource(...)`, and `render_prompt(...)` are the public resolved operations. They operate against the final server view, not the raw local/provider declaration set, which is why middleware, auth, visibility, and task handling belong conceptually to the server rather than to individual providers. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

---

## 4.8 Lifecycle entrypoints and deployment binding

### `run_async(...)` and `run(...)`

`TransportMixin` exposes `run_async(self, transport=None, show_banner=None, **transport_kwargs)` and `run(self, transport=None, show_banner=None, **transport_kwargs)`. Supported transport values are `stdio`, `http`, `sse`, and `streamable-http`; the user-facing running docs still frame STDIO as default, HTTP as Streamable HTTP, and SSE as legacy/deprecated. `run()` is synchronous and should not be called inside an async function; the running docs explicitly say to use `run_async()` there instead. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport), [gofastmcp.com](https://gofastmcp.com/deployment/running-server))

### `run_http_async(...)`

The transport SDK page shows the dedicated HTTP runner signature with `host`, `port`, `path`, `uvicorn_config`, `middleware`, `json_response`, and `stateless_http`-style controls. This is where transport binding belongs in v3, not in the constructor. If your docs or agent templates still put `host` or `port` into `FastMCP(...)`, they are structurally outdated. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

### `http_app(...)`

`http_app(...)` creates a Starlette application with the chosen HTTP transport. The SDK surface includes `path`, HTTP middleware, `json_response`, `stateless_http`, `transport`, optional `event_store`, and `retry_interval`. This is the correct binding point for production ASGI deployment, Starlette/FastAPI composition, or multi-worker hosting under Uvicorn/Gunicorn/Hypercorn. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport), [gofastmcp.com](https://gofastmcp.com/deployment/http))

### `if __name__ == "__main__":`

The running docs explicitly recommend wrapping `mcp.run()` in `if __name__ == "__main__":` for compatibility with MCP clients that launch the server as a subprocess. The CLI is the main exception: CLI-run servers do not need the block because the CLI discovers and launches the server object. ([gofastmcp.com](https://gofastmcp.com/deployment/running-server))

### Mounting / ASGI lifespan forwarding

When using `http_app()` inside FastAPI or Starlette, pass the returned app’s lifespan into the parent application. The FastAPI integration docs are explicit: omitting `lifespan=mcp_app.lifespan` prevents proper session-manager initialization. This is a lifecycle correctness issue, not a cosmetic best practice. ([gofastmcp.com](https://gofastmcp.com/integrations/fastapi))

```python id="rsocw3"
from fastapi import FastAPI
from fastmcp import FastMCP

mcp = FastMCP("Analytics")
mcp_app = mcp.http_app(path="/mcp")

app = FastAPI(lifespan=mcp_app.lifespan)
app.mount("/analytics", mcp_app)
```

### Lifespan semantics

Lifespans run once when the server starts and once when it stops, not per client session. The yielded dict becomes the lifespan context, accessible during tool execution. Cleanup should always live under `finally`. This is the correct place for DB pools, caches, task workers, model clients, and any other shared expensive state. ([gofastmcp.com](https://gofastmcp.com/servers/lifespan))

---

## 4.9 Tag filtering and visibility control

Tag filtering is not a constructor kwarg in v3; it is a post-construction server operation. The docs show the intended API: call `enable(...)` / `disable(...)` after server creation. Filtering applies across all component types—tools, resources, templates, and prompts—and affects both listing and access. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/servers/visibility), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

```python id="wkc7eh"
from fastmcp import FastMCP

mcp = FastMCP("Server")

mcp.enable(tags={"public"}, only=True)                 # allowlist mode
mcp.disable(tags={"internal", "deprecated"})          # block selected tags
mcp.disable(keys={"tool:delete_everything"})          # block exact component
mcp.enable(names={"admin_action"})                    # re-enable by name/URI
```

The visibility system works on keys, names/URIs, tags, versions, component types, and `match_all`. Component keys are typed: `tool:{name}`, `resource:{uri}`, `template:{uri}`, `prompt:{name}`. `enable(..., only=True)` switches the server into allowlist mode, clears previous allowlists, and leaves only matching components enabled; later `enable()` / `disable()` calls override earlier ones. ([gofastmcp.com](https://gofastmcp.com/servers/visibility))

Two architectural facts matter for agent authors. First, server-level visibility operates on the final cross-provider view, while provider-level visibility applies earlier and can be overridden by the server. Second, under the hood `enable()` / `disable()` are just convenience wrappers that add `Visibility` transforms; the server then applies the final filter after provider and server transforms complete. That is why visibility belongs conceptually in the transform/publication layer, not in component declaration. ([gofastmcp.com](https://gofastmcp.com/servers/visibility))

A third nuance: per-session visibility exists and overrides global defaults. `ctx.enable_components(...)`, `ctx.disable_components(...)`, and `ctx.reset_visibility()` let one session see a different server surface than another. That is not constructor state, but it is part of the lifecycle model because final visibility is session-aware at request time. ([gofastmcp.com](https://gofastmcp.com/servers/visibility), [gofastmcp.com](https://gofastmcp.com/servers/context))

---

## 4.10 Custom routes

`@mcp.custom_route(path, methods, name=None, include_in_schema=True)` registers extra HTTP routes on the same server alongside the MCP endpoint. The SDK is strict about the contract: the handler must be `async`, must accept a Starlette `Request`, and must return a `Response`. Best use cases are health checks, status endpoints, OAuth callbacks, and light admin APIs. For complex web applications, mount the server into a real ASGI app instead of overloading custom routes. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport), [gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/deployment/running-server))

```python id="xg8don"
from fastmcp import FastMCP
from starlette.requests import Request
from starlette.responses import JSONResponse

mcp = FastMCP("MyServer")

@mcp.custom_route("/health", methods=["GET"])
async def health_check(request: Request):
    return JSONResponse({"status": "ok"})
```

Operationally, custom routes are only meaningful on HTTP transports / `http_app()`. The running docs also note that custom routes sit at the domain root while the MCP endpoint lives at `/mcp/` by default. That distinction matters for reverse proxies, health checks, and ingress config. ([gofastmcp.com](https://gofastmcp.com/deployment/running-server))

---

## 4.11 Deployment advisory for constructor usage

**Rule 1:** treat `FastMCP(...)` as the **definition object**, not the **deployment object**. Put transport binding into `run_*()` or `http_app()`, never back into the constructor via stale templates. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport))

**Rule 2:** always set `name`, `version`, and `instructions` explicitly for production servers. The constructor can fill them implicitly, but explicit values stabilize client-visible identity and reduce documentation/test drift. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

**Rule 3:** prefer constructor-time `providers=[...]`, `transforms=[...]`, and `middleware=[...]` when topology is static and known at import time; prefer `add_provider()` / `add_transform()` / `add_middleware()` when composition is environment-driven or plugin-driven. Both are valid; constructor-time assembly is more declarative and easier to diff/review. ([gofastmcp.com](https://gofastmcp.com/servers/server), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

**Rule 4:** prefer `http_app()` + ASGI hosting for production HTTP deployments. The HTTP deployment docs explicitly position direct `run(transport="http")` as the fast path and ASGI as the higher-control path for multi-worker, middleware-rich, production-grade deployment. ([gofastmcp.com](https://gofastmcp.com/deployment/http))

**Rule 5:** treat constructor `auth=` as an HTTP concern. For STDIO, security is outside the FastMCP auth boundary. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

**Rule 6:** provide a real `session_state_store` for restart-tolerant or distributed deployments. The default in-memory store is fine for local dev and single-process servers; it is the wrong default for horizontally scaled or crash-sensitive infrastructure. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

**Rule 7:** leave `dereference_schemas=True` unless you have verified that every consuming client can handle `$ref`-heavy schemas. The docs explicitly say most clients need flat schemas. ([gofastmcp.com](https://gofastmcp.com/servers/server))

**Rule 8:** use `strict_input_validation=True` only when exact schema conformance matters more than compatibility with loosely typed clients. Flexible coercion is often the more practical production default for heterogeneous MCP hosts and agent stacks. ([gofastmcp.com](https://gofastmcp.com/servers/server))

---

## 4.12 Value case by constructor category

**Identity fields** make the server legible to clients and LLMs; without them, the server is operational but poorly steerable. ([gofastmcp.com](https://gofastmcp.com/servers/server))

**Composition fields** are the leverage layer: they let one `FastMCP` instance aggregate local declarations, dynamic providers, transforms, middleware, and shared startup state into a single published MCP surface. That is the transition from “Python module” to “server definition.” ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

**Behavior fields** tune compatibility, correctness, secrecy, scale, and client ergonomics: duplicate policy, validation strictness, masked failures, pagination, task defaults, client log verbosity, schema flattening. They are the “make this publishable in the real world” knobs. ([gofastmcp.com](https://gofastmcp.com/servers/server))

**Handlers/storage fields** are the server’s stateful edge: sampling fallback policy and session-state durability. They matter less in toy servers and much more in production orchestrators, long-lived assistants, and remote deployments. ([gofastmcp.com](https://gofastmcp.com/servers/server), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/server.py))

**Visibility/filtering** turns one server definition into multiple client views without rewriting component code. It is effectively runtime publication policy. ([gofastmcp.com](https://gofastmcp.com/servers/visibility))

**Custom routes** let one HTTP deployment expose both MCP and minimal web endpoints without forcing a separate web stack, but they are deliberately not a substitute for a real application framework. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport), [gofastmcp.com](https://gofastmcp.com/servers/server))

The next logical deep dive is **3) Running and deploying servers**, because after the constructor/model is fixed, the next agent-critical question is transport binding: `run()`, `run_async()`, `run_http_async()`, `http_app()`, STDIO vs Streamable HTTP vs SSE, ASGI mounting, lifespan propagation, path/base-url rules, and deployment-safe defaults.

# FastMCP Advanced — 5) Tools: definition, registration, and execution contract
### Tools: definition, registration, and execution contract

### 5.0 Role and runtime contract

Tools are the executable MCP surface: a Python callable becomes a client-callable capability with an input schema, validation behavior, execution path, and result-conversion pipeline. The official tool docs define the runtime sequence as: client/LLM sends arguments using the tool schema, FastMCP validates against the function signature, the function executes, and the result is returned to the client. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

For agent authors, the compact model is: **Python function → tool registration metadata → generated input schema → validated invocation → sync/async execution → result conversion to MCP `content` and optional `structuredContent` → error/timeout/version handling**. Return annotations and tool configuration materially affect the last two stages, so “tool definition” is not just naming; it is the full wire contract. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

### 5.1 Registration surfaces

FastMCP exposes three practical registration paths for tools:

1. `@mcp.tool` for direct declaration and immediate registration on a server.
2. `mcp.add_tool(...)` for programmatic registration of a `Tool` instance or callable.
3. standalone `@tool()` for attaching tool metadata to methods or reusable functions before later registration with `mcp.add_tool(...)`. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-tools-function_tool))

The current server SDK documents `add_tool(self, tool: Tool | Callable[..., Any]) -> Tool`, and the local-provider SDK says the provider-side `add_tool` accepts either a `Tool` object or a decorated function with FastMCP metadata. That means `add_tool(...)` is not limited to raw functions; it is the polymorphic registration entrypoint that normalizes both already-built tools and metadata-decorated callables into server-owned tool objects. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-providers-local_provider-decorators-tools))

### 5.2 `@mcp.tool` — direct registration

The documented minimal form is the bare decorator:

```python id="o8gv9p"
from fastmcp import FastMCP

mcp = FastMCP("Calculator")

@mcp.tool
def add(a: int, b: int) -> int:
    """Adds two integers."""
    return a + b
```

In that minimal form, FastMCP auto-infers the MCP tool name from the Python function name, the tool description from the docstring, and the input schema from the function signature plus type annotations. It also handles parameter validation and error reporting automatically. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

The server SDK documents these supported calling patterns for `mcp.tool(...)`: `@server.tool`, `@server.tool()`, `@server.tool("custom_name")`, `@server.tool(name="custom_name")`, and `server.tool(function, name="custom_name")`. This matters for code generation because `@mcp.tool` is both a decorator and a registration function. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

Representative forms:

```python id="iwnv1e"
@mcp.tool
def echo(text: str) -> str:
    return text

@mcp.tool()
def ping() -> str:
    return "pong"

@mcp.tool("search_catalog")
def search_impl(query: str) -> list[dict]:
    return []

@mcp.tool(name="search_catalog")
def search_impl2(query: str) -> list[dict]:
    return []

def external_fn(x: int) -> int:
    return x * 2

mcp.tool(external_fn, name="double")
```

Those are all first-class supported registration forms in the SDK surface. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

### 5.3 `mcp.add_tool(...)` — programmatic registration

`mcp.add_tool(...)` is the lower-level registration primitive. The server SDK describes it as accepting a `Tool` instance or an `@tool`-decorated function and returning the registered `Tool`. It is the correct API when components are assembled dynamically, when you are registering bound methods, or when metadata is attached out-of-band via the standalone decorator. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

```python id="8u44yo"
from fastmcp import FastMCP

mcp = FastMCP("Programmatic")

def ping() -> str:
    return "pong"

mcp.add_tool(ping)
```

Internally, the source-level coercion rule is: if the argument is already a `Tool`, keep it; otherwise inspect the callable for attached FastMCP `ToolMeta`; if metadata is present, build the runtime tool with `FunctionTool.from_function(tool, metadata=fmeta)`; if not, fall back to `Tool.from_function(tool)`. That is the exact mechanism that makes standalone-decorated methods and reusable functions work correctly with `mcp.add_tool(...)`. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

### 5.4 Standalone `@tool()` — metadata attachment without immediate registration

The standalone decorator lives in `fastmcp.tools.function_tool`. Its SDK page defines it as a decorator that marks a function as an MCP tool, **returns the original function with metadata attached**, and is intended to be registered later with `mcp.add_tool()`. This is the correct primitive for methods and reusable callables that should remain ordinary Python functions until bound or composed. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-tools-function_tool))

```python id="nq77gv"
from fastmcp import FastMCP
from fastmcp.tools import tool

class Calculator:
    def __init__(self, multiplier: int):
        self.multiplier = multiplier

    @tool()
    def multiply(self, x: int) -> int:
        """Multiply by the instance multiplier."""
        return x * self.multiplier

calc = Calculator(multiplier=3)
mcp = FastMCP("Calc")
mcp.add_tool(calc.multiply)
```

The tools guide is explicit about why this exists: `@mcp.tool` registers immediately, which is wrong for instance/class methods because the unbound signature would expose `self` or `cls` as required tool parameters. With `@tool()` plus later registration of the **bound** method, FastMCP sees the correct signature and registers only the user-supplied arguments. The guide’s example states this directly for `mcp.add_tool(calc.multiply)`, which registers only `x`, not `self`. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

There is one additional v3 nuance that matters for agent-authored code: decorators now return the original function by default rather than a component object. The upgrade guide and settings docs both say `FASTMCP_DECORATOR_MODE=function` is the default; `object` compatibility mode exists but is deprecated. That default applies to `@mcp.tool` as well, which is why decorated functions remain directly callable in tests and non-MCP code paths. ([gofastmcp.com](https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2), [gofastmcp.com](https://gofastmcp.com/more/settings))

### 5.5 Supported callable forms: sync and async

FastMCP supports both synchronous `def` and asynchronous `async def` tool functions. The tools guide states that sync tools automatically run in a threadpool so they do not block the event loop, while async tools are still preferred for I/O-bound work because they are more efficient than threadpool dispatch. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

```python id="pux5ey"
import time
from fastmcp import FastMCP

mcp = FastMCP("Concurrency")

@mcp.tool
def slow_sync(x: int) -> int:
    time.sleep(2)
    return x * 2

@mcp.tool
async def fast_async(x: int) -> int:
    return x * 3
```

The operational guidance is therefore precise: use `async def` for network/database/file-async libraries and long-lived concurrent I/O; use `def` for simple CPU-light logic or when integrating synchronous libraries. FastMCP will still execute synchronous tools concurrently via threadpool dispatch. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.6 Exact decorator/configuration surface

At the docs layer, FastMCP prominently documents these `@mcp.tool(...)` arguments: `name`, `description`, `tags`, `enabled` (deprecated in v3), `icons`, `annotations`, `meta`, `timeout`, `version`, and `output_schema`. The docs further explain the semantics of each, including that `description` overrides the function docstring, `tags` support organization/filtering, `meta` is static tool-definition metadata, `timeout` is foreground execution timeout in seconds, `version` identifies tool versions, and `output_schema` declares structured output shape. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

The current source-level tool-construction surface is slightly broader. `Tool.from_function(...)` currently accepts `name`, `version`, `title`, `description`, `icons`, `tags`, `annotations`, `exclude_args`, `output_schema`, deprecated `serializer`, `meta`, `task`, `timeout`, and `auth`, then forwards that configuration into `FunctionTool.from_function(...)`. This is the authoritative current parameter set for the runtime tool factory. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

For agent documentation, the most useful normalized view is:

* **identity**: `name`, `version`, optional source-level `title`
* **LLM/client description**: `description`, `annotations`
* **organization / exposure**: `tags`, `icons`, `meta`
* **execution policy**: `timeout`, `task`, `auth`
* **schema policy**: `output_schema`, deprecated `exclude_args`, deprecated `serializer` ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py), [gofastmcp.com](https://gofastmcp.com/servers/authorization))

Representative dense form:

```python id="psca8f"
from fastmcp import FastMCP
from fastmcp.server.auth import require_scopes
from mcp.types import ToolAnnotations

mcp = FastMCP("Catalog")

@mcp.tool(
    name="find_products",
    description="Search catalog items by query and optional category filter.",
    tags={"catalog", "search"},
    meta={"team": "commerce", "stability": "ga"},
    timeout=15.0,
    version="2.1",
    output_schema={
        "type": "object",
        "properties": {
            "items": {"type": "array"},
            "total": {"type": "integer"},
        },
        "required": ["items", "total"],
    },
    annotations=ToolAnnotations(
        title="Find Products",
        readOnlyHint=True,
        openWorldHint=False,
    ),
    auth=require_scopes("catalog:read"),
)
async def search_products(query: str, category: str | None = None) -> dict:
    return {"items": [], "total": 0}
```

That example is valid in the sense documented by the tools and authorization surfaces: component-level auth is supported, `timeout` and `version` are v3 tool features, annotations are advisory MCP metadata, and `output_schema` is explicit structured-output control. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/servers/authorization))

### 5.7 Per-argument semantics

#### `name`

If omitted, FastMCP uses the Python function name as the exposed tool name. Tool names are later validated against MCP naming rules by the tool model validator in source. In practice, explicit `name=` should be used only when the Python implementation name is not the desired public contract. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

#### `description`

If omitted, FastMCP uses the function docstring. If provided, the explicit description wins and the function docstring is ignored for tool-description purposes. Best practice: keep implementation docstrings terse/internal, and use `description=` only when the model-facing description must diverge from internal developer documentation. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

#### `tags`

Tags are organizational metadata that the server and some clients can use for grouping/filtering. They also matter operationally because server-level visibility and auth patterns can target tags, so tags are not purely decorative. Prefer stable domain tags like `{"admin"}`, `{"read"}`, `{"billing"}`, `{"experimental"}` over ephemeral labels. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/servers/visibility), [gofastmcp.com](https://gofastmcp.com/servers/authorization))

#### `meta`

The tools guide defines `meta` on the decorator as static metadata about the tool definition and distinguishes it from `ToolResult.meta`, which is runtime metadata about a specific execution. This distinction is crucial: definition metadata belongs on `@mcp.tool(meta=...)`; per-call metrics/debug info belong on `ToolResult(meta=...)`. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

#### `timeout`

`timeout` is a foreground execution timeout in seconds. If exceeded, the client receives an MCP error; the tools guide states the error code is `-32000` and that both sync and async tools support timeouts. There is no server-level default; timeouts must be opted into tool by tool. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

#### `version`

The tools guide says v3 tools support versioning and that clients automatically receive the highest version by default. The public tool/decorator surface and source both accept `version`, which means versioning is a first-class tool identity dimension, not a convention hidden inside `meta`. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

#### `output_schema`

`output_schema` is explicit JSON Schema control for structured output. If provided, the tool must return structured output matching it; if omitted, FastMCP tries to infer an output schema from the return annotation. The tools guide also states an important MCP constraint: output schemas must be object types. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

#### `annotations`

Annotations are advisory MCP metadata such as `title`, `readOnlyHint`, `destructiveHint`, `idempotentHint`, and `openWorldHint`. They affect client UX and safety presentation rather than security enforcement. In current source, tool serialization prefers `self.title` if set, otherwise `annotations.title`; in practice, portable client-facing display titles are best expressed through annotations. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

#### `auth`

Component-level `auth` is supported in the current source-level tool factory and documented in the authorization guide through examples like `@mcp.tool(auth=require_scopes("admin"))`. Component-level auth both filters list visibility and enforces direct-access permissions for the current request. Authorization applies only in HTTP transports; STDIO bypasses OAuth/token-based auth checks. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py), [gofastmcp.com](https://gofastmcp.com/servers/authorization))

#### `task`

The tools guide documents `@mcp.tool(task=True)` for background execution. In current source, `task` is part of the tool-construction surface. This is not just a hint; it changes execution routing into Docket-backed task execution instead of normal foreground tool completion. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

#### Deprecated knobs

`enabled` is deprecated in v3 in favor of server-level `mcp.enable()` / `mcp.disable()`. `exclude_args` is deprecated in favor of dependency injection with `Depends()`. `serializer` still exists at the source/tool-model layer, but the docs explicitly recommend returning `ToolResult` for full serialization control instead. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-providers-local_provider-decorators-tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

### 5.8 What FastMCP auto-infers from the Python function

FastMCP auto-infers at least four major pieces of MCP contract data from the function itself: tool name from the Python symbol, description from the docstring, input schema from signature + type hints, and output schema from the return annotation when no explicit `output_schema` is supplied. Required vs optional parameters also follow normal Python default-value semantics, and parameter-level descriptions/constraints can be derived from `Annotated[...]`, plain string annotations, and `pydantic.Field(...)`. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

Parameter metadata inference is richer than “just types.” The tools guide documents `Annotated[str, "description"]` shorthand, `Field(description=..., ge=..., le=..., pattern=..., min_length=..., max_length=...)`, and default values as inputs to the generated schema. That means the correct authoring stance is: treat the Python signature as the primary schema source, and use Pydantic field metadata rather than out-of-band schema postprocessing whenever possible. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

Special parameter forms matter. A parameter annotated as `Context` is treated as MCP runtime context access, not an LLM-supplied argument; parameters using `Depends(...)` are automatically excluded from the tool schema and injected at runtime. This is the preferred mechanism for hidden runtime dependencies such as user IDs, credentials, DB handles, or request-scoped services. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server), [gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.9 Why `*args` / `**kwargs` are unsupported

The tools guide is explicit: functions with `*args` or `**kwargs` are not supported as tools because FastMCP must generate a complete parameter schema for the MCP protocol, and a variadic signature cannot be expressed as a stable, fully specified MCP input schema. This is not an arbitrary framework restriction; it is a direct consequence of MCP’s need for precise machine-readable tool schemas. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

The practical design rule is: expose a fixed keyword-argument contract to the model. If your internal implementation wants variadic flexibility, hide that behind a stable wrapper function or a structured object parameter. For agent-authored code, that usually means “one explicit wrapper tool per user-facing action,” not “forward arbitrary kwargs into an internal SDK.” ([gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.10 Execution contract: validation, invocation, and result conversion

On invocation, FastMCP validates tool inputs against the generated schema/signature and then executes the function with validated values. Validation is flexible by default—stringified numerics and booleans may be coerced—unless the server enables `strict_input_validation=True`, in which case exact JSON Schema validation runs before the tool function is called. The tools guide recommends flexible validation for most real LLM-client use cases because many clients send stringified values. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

Return conversion has two layers: traditional MCP content blocks and optional structured output. The tools guide states that all results are converted to content blocks for backward compatibility, while object-like results such as dicts, dataclasses, and Pydantic models also become structured content automatically. Non-object results such as primitives and collections only become structured content when there is an output schema, typically derived from a return annotation or supplied explicitly. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

The current source gives the precise primitive-wrapping behavior: when `output_schema` exists for a primitive-like return, FastMCP may wrap the serialized value under `{"result": ...}` so the structured output remains an object-shaped JSON document, which matches the tools guide’s documented primitive wrapping rule. The same source also shows that `ToolResult` is a full passthrough escape hatch: if the raw return is already a `ToolResult`, FastMCP does not auto-wrap or reinterpret it. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

Automatic content conversion is type-sensitive. The tools guide documents these mappings: `str` to `TextContent`, `bytes` to embedded/base64 resource content, FastMCP `Image`/`Audio`/`File` helpers to corresponding MCP media/resource content, MCP content blocks passed through as-is, lists converted itemwise, and `None` producing an empty response. This means return-type design affects both model-visible text and machine-readable payloads. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.11 `ToolResult` — full-control response mode

When automatic conversion is insufficient, return `ToolResult`. The docs define it as the full-control response object with `content`, `structured_content`, and `meta`, and the current source enforces that at least one of `content` or `structured_content` must be provided. The docs further say `structured_content` must be a dictionary or `None`, and that if only `structured_content` is provided it is also used as the basis for generated content. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

```python id="sudr65"
from fastmcp.tools.tool import ToolResult
from mcp.types import TextContent

@mcp.tool
def advanced_tool() -> ToolResult:
    return ToolResult(
        content=[TextContent(type="text", text="Human-readable summary")],
        structured_content={"data": "value", "count": 42},
        meta={"execution_time_ms": 145},
    )
```

Use `ToolResult` when you need exact control over human-readable content, exact structured payload shape, or per-execution metadata. Use raw Python returns when the default conversion rules are acceptable and you want maximum brevity. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.12 Errors, timeouts, and background-task boundary

The tools guide says ordinary Python exceptions and FastMCP `ToolError` are both valid failure paths. By default, exceptions are logged and converted into MCP error responses; with `mask_error_details=True` at the server, generic exceptions are masked, but `ToolError` messages still propagate to clients. That makes `ToolError` the right mechanism for intentional, user-facing tool failures. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

Timeouts apply only to foreground execution. The tools guide is explicit that `timeout` does **not** apply when the tool runs as a background task via `task=True`; background timeouts must be enforced through Docket’s own timeout mechanisms. The guide also recommends task mode for known long-running jobs because FastMCP will otherwise treat a long foreground execution as a timeout-worthy failure. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

### 5.13 Best-practice authoring rules for agents

Prefer **explicit, fixed, keyword-shaped signatures** with rich type hints and `Field(...)` metadata. This yields better schema generation, better client interoperability, and fewer downstream transform hacks. Avoid variadics, implicit JSON-in-string arguments, and overly clever Python signatures that do not map cleanly to stable MCP input schemas. ([gofastmcp.com](https://gofastmcp.com/servers/tools))

Use `@mcp.tool` for ordinary top-level functions; use standalone `@tool()` plus `mcp.add_tool(bound_method)` for instance/class methods or reusable function libraries. This is the only robust way to keep `self`/`cls` out of the published schema while preserving normal Python callability. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-tools-function_tool))

Prefer return annotations and ordinary object returns when automatic structured output is sufficient; use explicit `output_schema` only when the inferred schema is not the public contract you want. Prefer `ToolResult` over deprecated serializer-based customization when you need exact response control. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/tools/tool.py))

Prefer `Depends(...)` for hidden runtime parameters and `Context` for MCP capabilities; do not expose infrastructure-only inputs as public tool arguments. The tool schema should represent what the client/LLM is actually expected to supply, not what the runtime needs internally. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-server))

Prefer server-level visibility control over deprecated per-tool `enabled=`. Prefer explicit `version=` when maintaining parallel implementations under one public tool name. Prefer `auth=` only on HTTP-facing servers where token-based authorization actually exists. ([gofastmcp.com](https://gofastmcp.com/servers/tools), [gofastmcp.com](https://gofastmcp.com/servers/authorization))

The next logical deep dive is **5) Tools: typing, validation, hidden parameters, outputs, and content blocks**, because once the registration/execution surface is fixed, the next agent-critical layer is the schema engine itself: parameter typing, `Annotated`/`Field`, `Depends`, `Context`, output-schema generation, `ToolResult`, media/file return helpers, and exact result-shaping rules.

# FastMCP Advanced — 6) Tools: typing, validation, hidden parameters, outputs, and content blocks
### Tools: typing, validation, hidden parameters, outputs, and content blocks

### Version anchors used here

The relevant feature gates for this section are spread across several releases: output schemas and structured content in `2.10.0`, `Annotated[str, "description"]` shorthand in `2.11.0`, flexible-vs-strict input validation in `2.13.0`, `ToolResult.meta` in `2.13.1`, `Depends()`-based hidden parameters in `2.14.0`, `CurrentContext()` as the preferred explicit context dependency in `2.14`, and per-tool `timeout=` plus `version=` in `3.0.0`. The docs also note that serve-time schema dereferencing is on by default unless the server disables it with `dereference_schemas=False`. ([FastMCP][1])

### 6.0 Contract summary: what the tool signature controls

A FastMCP tool signature is the primary schema source. FastMCP inspects the function name, docstring, parameter list, defaults, and type annotations to derive the MCP-visible name, description, required-vs-optional argument set, and input schema; on execution it validates arguments against that contract before calling the function. Functions using `*args` or `**kwargs` are rejected because FastMCP must emit a complete parameter schema for MCP, and variadic signatures do not map to a stable machine-readable contract. ([FastMCP][1])

The consequence for agent authors is strict: the public tool API should be modeled as a fixed, schema-friendly Python function, not as a thin wrapper over an internal variadic API. If the internal implementation wants dynamic argument handling, put that behind a stable, explicitly typed MCP-facing wrapper. ([FastMCP][1])

### 6.1 Argument schema generation from type hints

FastMCP generates tool input schemas from the function signature and Python type annotations. The tools docs explicitly call out support for standard scalar types (`int`, `float`, `str`, `bool`), binary `bytes`, date/time types, collections like `list[str]` and `dict[str, int]`, optionals and unions, constrained types such as `Literal[...]` and `Enum`, `Path`, `UUID`, and Pydantic models; the same section notes that FastMCP supports the full set of types that Pydantic supports as fields. ([FastMCP][1])

Representative authoring shape:

```python id="2fup92"
from pathlib import Path
from typing import Literal
from uuid import UUID
from fastmcp import FastMCP

mcp = FastMCP("TypedTools")

@mcp.tool
def ingest_document(
    file: Path,
    tenant_id: UUID,
    mode: Literal["fast", "accurate"] = "fast",
    retries: int = 3,
) -> dict:
    return {"ok": True}
```

Requiredness follows ordinary Python rules: parameters without defaults are required; parameters with defaults are optional; `None`-typed optionals remain optional in the generated schema. ([FastMCP][1])

Two subtle but important type-behavior notes are documented. First, `bytes` parameters accept raw strings and are **not** automatically base64-decoded; if the client will send base64, use `str` and decode manually. Second, Pydantic-model parameters must be provided as JSON objects, not as stringified JSON, even in flexible validation mode. ([FastMCP][1])

### 6.2 Serve-time schema shaping and `$ref` dereferencing

The tools docs state that FastMCP automatically dereferences `$ref` entries in tool schemas at serve time by default, specifically to improve compatibility with MCP clients that do not fully support JSON Schema references. Schemas are stored with `$ref` intact internally and flattened only when sent to clients; opting out is done at the server level with `FastMCP(..., dereference_schemas=False)`. ([FastMCP][1])

Operationally, that means the Python signature remains the canonical schema source, but the wire-level schema seen by clients may be a serve-time flattened variant. Agent-authored tests that compare server-emitted schemas should account for this distinction instead of assuming that internal Pydantic/OpenAPI-style `$defs` survive transport unchanged. ([FastMCP][1])

### 6.3 Validation modes: flexible vs strict

Validation mode is server-level, not per-tool. The tools docs say the default is flexible Pydantic validation, which coerces compatible values to the annotated type—for example `"10"` to `int`, `"3.14"` to `float`, `"true"` to `bool`, and lists of string numerics to `list[int]`. Strict mode is enabled via `FastMCP(..., strict_input_validation=True)` and uses the MCP SDK’s JSON Schema validation to reject such mismatches before the function runs. ([FastMCP][1])

That yields a crisp decision rule. Use flexible mode for most real MCP deployments because LLM clients often stringify values. Use strict mode only when exact wire-type conformance matters more than client leniency—for example, compliance-sensitive systems or test harnesses validating a third-party client implementation. ([FastMCP][1])

Minimal strict-mode example:

```python id="hh8fht"
from fastmcp import FastMCP

mcp = FastMCP("StrictServer", strict_input_validation=True)

@mcp.tool
def add_numbers(a: int, b: int) -> int:
    return a + b
```

Under that policy, `{"a": "10", "b": "20"}` fails validation instead of being coerced. ([FastMCP][1])

### 6.4 Parameter metadata: `Annotated[...]` shorthand and `Field(...)`

FastMCP supports two main metadata layers on parameters. For lightweight descriptions, the docs support `Annotated[T, "description"]`; this is equivalent to `Field(description=...)` but only when the `Annotated` metadata is a single string. For richer contracts, use `Annotated[T, Field(...)]`, which supports both documentation and validation constraints. The docs also note that `Field(...)` as a default value works, but the `Annotated[...]` form is preferred. ([FastMCP][1])

Shorthand description form:

```python id="h9pqn6"
from typing import Annotated
from fastmcp import FastMCP

mcp = FastMCP("Metadata")

@mcp.tool
def process_image(
    image_url: Annotated[str, "URL of the image to process"],
    resize: Annotated[bool, "Whether to resize the image"] = False,
    width: Annotated[int, "Target width in pixels"] = 800,
) -> dict:
    return {"ok": True}
```

Rich validation/documentation form:

```python id="y0m0uy"
from typing import Annotated, Literal
from pydantic import Field
from fastmcp import FastMCP

mcp = FastMCP("Metadata")

@mcp.tool
def process_image(
    image_url: Annotated[str, Field(description="URL of the image to process")],
    width: Annotated[int, Field(description="Target width in pixels", ge=1, le=2000)] = 800,
    format: Annotated[
        Literal["jpeg", "png", "webp"],
        Field(description="Output image format"),
    ] = "jpeg",
) -> dict:
    return {"ok": True}
```

The documented `Field(...)` features include `description`, `ge` / `gt` / `le` / `lt`, `min_length` / `max_length`, `pattern`, and `default`. For schema-oriented tool authoring, `Field(...)` is the highest-leverage way to encode argument contracts without hand-writing JSON schema. ([FastMCP][1])

### 6.5 Hidden parameters and dependency injection

FastMCP’s rule for hidden runtime-only parameters is simple: if a parameter is recognized as a dependency, it is excluded from the MCP schema and injected at runtime. The dependency-injection docs state this globally, and the tools page explicitly recommends `Depends()` for runtime-only values like user IDs, credentials, and DB connections. ([FastMCP][2])

Canonical `Depends()` pattern:

```python id="4401oj"
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("HiddenDeps")

def get_user_id() -> str:
    return "user_123"

@mcp.tool
def get_user_details(user_id: str = Depends(get_user_id)) -> str:
    return f"Details for {user_id}"
```

Here `user_id` is injected by the server and does not appear in the MCP-exposed callable schema. That is the preferred modern replacement for the older `exclude_args` pattern. The public server SDK still exposes `exclude_args`, but marks it deprecated and directs users toward `Depends()` instead. ([FastMCP][1])

### 6.6 Context as a hidden injected parameter

`Context` is also a dependency parameter and therefore hidden from the tool schema. The dependency-injection docs state that a parameter annotated as `Context` is injected automatically, and the context page says the preferred explicit form since `2.14` is `ctx: Context = CurrentContext()`. The context page also states that context methods are async, that each request gets a fresh context object, and that context is only valid during a request. ([FastMCP][2])

Preferred explicit form:

```python id="n5czz7"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

mcp = FastMCP("ContextDemo")

@mcp.tool
async def process_file(file_uri: str, ctx: Context = CurrentContext()) -> str:
    await ctx.info(f"Processing {file_uri}")
    return "Processed file"
```

Legacy type-hint-only injection still works:

```python id="xv9wtg"
from fastmcp import FastMCP, Context

mcp = FastMCP("ContextDemo")

@mcp.tool
async def process_file(file_uri: str, ctx: Context) -> str:
    return "Processed file"
```

The explicit `CurrentContext()` form is better for agent-generated code because the hidden dependency is visible in the signature, while still staying خارج the tool schema seen by clients. ([FastMCP][3])

### 6.7 Timeouts

Per-tool timeouts are configured with `@mcp.tool(timeout=<seconds>)`. The tools docs say this feature is new in `3.0.0`, that the timeout is expressed as a float in seconds, that both sync and async tools support it, and that expiration returns an MCP error with code `-32000`. The docs are also explicit that there is no server-level default timeout; tools must opt in individually. ([FastMCP][1])

```python id="jlwm5h"
from fastmcp import FastMCP

mcp = FastMCP("Timeouts")

@mcp.tool(timeout=30.0)
async def fetch_data(url: str) -> dict:
    return {"url": url}
```

Timeouts only apply to foreground execution. The tools docs explicitly say `timeout` does **not** apply to background-task execution (`task=True`); for task time limits, use Docket’s own timeout mechanisms instead. That distinction is easy to miss and should be encoded directly into agent templates for long-running operations. ([FastMCP][1])

### 6.8 Return model: content blocks + structured content + output schema

FastMCP’s return contract has three layers: the raw Python return value, the MCP `content` blocks shown to users/models, and optional machine-readable `structuredContent`. The tools docs say all tool results are always converted into traditional content blocks for backward compatibility, while structured output is emitted when the return value has an object representation or when an output schema is present. ([FastMCP][1])

The documented automatic structured-output rules are:

* object-like returns (`dict`, dataclasses, Pydantic models) always become structured content, even without an explicit output schema;
* non-object returns (`int`, `str`, `list`) only become structured content when an output schema exists, typically via a return annotation or explicit `output_schema`;
* all results still become traditional content blocks. ([FastMCP][1])

This distinction is the key authoring rule for tool returns. If you want machine-readable output by default, prefer object-shaped returns. Primitive-only returns are fine, but they need a return annotation or explicit output schema to participate in the structured-output channel. ([FastMCP][1])

### 6.9 Automatic output-schema generation from return annotations

FastMCP automatically generates output schemas from return type annotations. The tools docs say this works for basic types, collections, unions, dataclasses, TypedDicts, and Pydantic models. For dataclasses and Pydantic models specifically, FastMCP extracts field definitions to produce detailed schemas; clients then receive both `content` and `structuredContent`. ([FastMCP][1])

Typed-model example:

```python id="s0w727"
from dataclasses import dataclass
from fastmcp import FastMCP

mcp = FastMCP("Profiles")

@dataclass
class Person:
    name: str
    age: int
    email: str

@mcp.tool
def get_user_profile(user_id: str) -> Person:
    return Person(name="Alice", age=30, email="alice@example.com")
```

For primitive returns, FastMCP wraps the value under a `"result"` key so the structured output remains a valid object-shaped JSON document, since output schemas must have an object root. The docs show this explicitly for return types like `-> int`. ([FastMCP][1])

```python id="s6k1tf"
@mcp.tool
def calculate_sum(a: int, b: int) -> int:
    return a + b
```

If you remove the return annotation entirely for a primitive-like result, the tool still produces `content`, but not structured output. That difference is explicitly documented and is a common source of confusion in downstream client integrations. ([FastMCP][1])

### 6.10 Manual `output_schema={...}`

Manual schema override is done with `@mcp.tool(output_schema={...})`. The tools docs say that when `output_schema` is supplied, the tool must return structured output matching that schema; they also state the schema root must be an object, not a primitive or array root. ([FastMCP][1])

```python id="6l12ud"
@mcp.tool(
    output_schema={
        "type": "object",
        "properties": {
            "data": {"type": "string"},
            "metadata": {"type": "object"},
        },
        "required": ["data", "metadata"],
    }
)
def custom_schema_tool() -> dict:
    return {"data": "Hello", "metadata": {"version": "1.0"}}
```

Use explicit `output_schema` when the inferred return schema is not the public contract you want—for example, when a loose `dict[str, Any]` implementation should still expose a narrow client-facing schema, or when you want to freeze a tool contract even as internal model classes evolve. The docs also note that `ToolResult` can still supply structured output even without an explicit output schema, but validation/documentation are strongest when the schema is declared. ([FastMCP][1])

### 6.11 `ToolResult` — full manual control

For full control over tool responses, return `ToolResult`. The tools docs define its public fields as `content`, `structured_content`, and `meta`; they also state that at least one of `content` or `structured_content` must be provided, that `structured_content` must be a dictionary or `None`, and that if only `structured_content` is provided it is also used as `content` after JSON-string conversion. The current source matches that contract and enforces the “at least one of the two” invariant in `ToolResult.__init__`. ([FastMCP][1])

```python id="cg2r8m"
from fastmcp.tools.tool import ToolResult
from mcp.types import TextContent

@mcp.tool
def advanced_tool() -> ToolResult:
    return ToolResult(
        content=[TextContent(type="text", text="Human-readable summary")],
        structured_content={"data": "value", "count": 42},
        meta={"execution_time_ms": 145},
    )
```

`ToolResult.meta` is runtime metadata about a specific execution, not static definition metadata. The tools docs explicitly distinguish this from `@mcp.tool(meta={...})`, which describes the tool definition itself. The docs also say that when returning `ToolResult`, FastMCP does not automatically wrap or transform the payload, and that `ToolResult` can be used with or without an output schema. ([FastMCP][1])

For agent authors, the decision rule is simple. Use raw Python returns for default behavior and low ceremony. Use `ToolResult` when you need exact control over the `content`/`structuredContent` split, when you need execution metadata, or when you want to bypass the framework’s automatic wrapping rules. ([FastMCP][1])

### 6.12 Content-block conversion rules

The tools docs define the automatic content-block mapping as follows: `str` becomes `TextContent`; `bytes` becomes base64-encoded embedded resource content; `Image` becomes `ImageContent`; `Audio` becomes `AudioContent`; `File` becomes an embedded resource; MCP SDK content blocks pass through unchanged; `None` yields an empty response. Lists of supported content/media items are also supported. ([FastMCP][1])

That conversion layer is independent of structured output. Even when a return value also generates `structuredContent`, FastMCP still emits content blocks for backward compatibility. This is why raw dict/model returns can satisfy both model-facing and machine-facing clients simultaneously. ([FastMCP][1])

### 6.13 `Image`, `Audio`, and `File` helper return types

FastMCP ships helper classes in `fastmcp.utilities.types` for binary/media returns. The tools docs say returning `Image`, `Audio`, or `File` directly—or as part of a list—causes automatic conversion into the appropriate MCP content block. The utilities SDK documents the corresponding manual-conversion helpers: `Image.to_image_content(...)`, `Audio.to_audio_content(...)`, and `File.to_resource_content(...)`. ([FastMCP][1])

```python id="7wp5wy"
from fastmcp.utilities.types import Image, Audio, File

@mcp.tool
def get_chart() -> Image:
    return Image(path="chart.png")

@mcp.tool
def get_clip() -> Audio:
    return Audio(path="sample.wav")

@mcp.tool
def get_report() -> File:
    return File(path="report.pdf")
```

The tools docs also define the constructor contract for these helpers: use either `path=` or `data=` (mutually exclusive); `data=` requires `format=` so MIME type can be determined; `File` additionally supports `name=` when using `data=`; all helper types support optional content `annotations`. ([FastMCP][1])

One important caveat is also explicit in the docs: helper classes are only auto-converted when returned directly or as part of a list. They are **not** automatically converted when nested inside containers like dicts. For nested/object use, manually call `.to_image_content()`, `.to_audio_content()`, or `.to_resource_content()`. ([FastMCP][1])

```python id="wzl9n5"
from fastmcp.utilities.types import Image

@mcp.tool
def nested_media() -> dict:
    return {
        "image": Image(path="chart.png").to_image_content()
    }
```

### 6.14 Authoring patterns that produce stable MCP contracts

Pattern 1: for machine-consumable tools, prefer object-shaped return types—dicts, dataclasses, Pydantic models, TypedDicts—because they automatically participate in structured output and generate richer schemas. Primitive-only returns are concise, but they need explicit return annotations or manual `output_schema` to become structured. ([FastMCP][1])

Pattern 2: use `Annotated[..., Field(...)]` as the default parameter-contract idiom. It keeps the Python signature readable while allowing descriptions, bounds, regexes, and default semantics to live in one place. Use `Annotated[..., "description"]` only for the lightweight case where you need prose but no constraints. ([FastMCP][1])

Pattern 3: hide runtime-only values with DI, not with public parameters. `Depends()`, `CurrentContext()`, and recognized injected dependency types keep the MCP schema aligned with what the LLM/client is actually expected to provide. The docs explicitly recommend this and deprecate `exclude_args` in favor of DI. ([FastMCP][2])

Pattern 4: reserve `ToolResult` for cases that truly need manual control. Overusing it removes some of the benefits of automatic schema/content generation; underusing it forces awkward hacks when you really need distinct human-facing text, structured data, and metadata in one response. ([FastMCP][1])

Pattern 5: put long-running foreground work behind `timeout=` and put truly long-running workflows behind `task=True`. The docs explicitly separate those two execution models and say foreground timeouts do not govern background tasks. ([FastMCP][1])


[1]: https://gofastmcp.com/servers/tools "Tools - FastMCP"
[2]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection - FastMCP"
[3]: https://gofastmcp.com/servers/context "MCP Context - FastMCP"

# FastMCP Advanced — 7) Resources and resource templates
### Resources and resource templates

### 7.0 Resource model: what a resource is, and when it is the right abstraction

Resources are the MCP read-side surface: they expose **read-only** data or files addressable by URI, and when a client requests a resource URI FastMCP resolves the definition, executes the backing function if the resource is function-backed, and returns text, JSON-like content, or binary data. Resource templates extend the same model by letting one URI pattern represent a family of on-demand resources parameterized by URI values. Use a resource when the capability is fundamentally “retrieve data at a stable address,” not “perform an imperative action”; use a template when the address pattern is stable but the concrete instance is parameterized. ([FastMCP][1])

### 7.1 `@mcp.resource(...)` — primary declaration surface

The primary declaration form is `@mcp.resource(uri, ...)`. The decorator requires a resource URI and supports metadata fields including `name`, `description`, `mime_type`, `tags`, `icons`, `annotations`, `meta`, and `version`; `enabled` still appears in the docs but is deprecated in v3 in favor of server-level `mcp.enable()` / `mcp.disable()`. Name defaults to the Python function name, description defaults to the docstring, and `mime_type` is inferred when possible but should be set explicitly for nontrivial content types. ([FastMCP][1])

```python id="y0jjxt"
import json
from fastmcp import FastMCP

mcp = FastMCP(name="DataServer")

@mcp.resource(
    uri="data://app-status",
    name="ApplicationStatus",
    description="Provides the current status of the application.",
    mime_type="application/json",
    tags={"monitoring", "status"},
    meta={"version": "2.1", "team": "infrastructure"},
)
def get_application_status() -> str:
    return json.dumps({"status": "ok"})
```

At the SDK level, `FastMCP.resource(...)` is documented as a decorator that registers a function as a resource, accepts sync or async functions, returns text from `str`, binary from `bytes`, and otherwise treats the callable as a resource generator. It also explicitly allows a `Context` parameter and states that if the URI contains parameters or the function has parameters, the registration becomes a **template resource** rather than a simple static resource definition. ([FastMCP][2])

### 7.2 Static resources, function-backed resources, and templates: object-model split

FastMCP has three practically distinct resource forms. First, **function-backed fixed-URI resources**: a normal `@mcp.resource("scheme://fixed-uri")` function that runs only when the resource is read. Second, **concrete resource objects** such as `TextResource`, `FileResource`, and `DirectoryResource`, registered with `mcp.add_resource(...)`; these are the preferred form for predefined static content and direct file/URL-backed content. Third, **resource templates**, where the URI contains placeholders like `{id}` or `{path*}` and each unique parameter set generates a concrete resource on demand. The docs emphasize that `@mcp.resource` is ideal for dynamic content, while concrete `Resource` subclasses are the direct-registration path for predefined/static content. ([FastMCP][1])

The lazy-loading property is not incidental. The `FunctionResource` SDK page states that a function-backed resource defers data loading until the resource is actually read, which is particularly important because resource listing does not eagerly execute the function. That gives function-backed resources a strong value case when the content is expensive to compute but should still be discoverable by clients via `resources/list`. ([FastMCP][3])

### 7.3 `mcp.add_resource(...)` and direct registration

`FastMCP.add_resource(self, resource: Resource | Callable[..., Any]) -> Resource | ResourceTemplate` is the programmatic registration API. It accepts either a concrete `Resource` instance or an `@resource`-decorated function and returns the normalized registered object; if the callable represents a template, the return type is a `ResourceTemplate`, not a plain `Resource`. For direct template registration there is also `add_template(self, template: ResourceTemplate) -> ResourceTemplate`. ([FastMCP][2])

```python id="a4qq5x"
from pathlib import Path
from fastmcp import FastMCP
from fastmcp.resources import TextResource, FileResource

mcp = FastMCP("StaticResources")

notice = TextResource(
    uri="resource://notice",
    name="Important Notice",
    text="System maintenance scheduled for Sunday.",
    tags={"notification"},
)
mcp.add_resource(notice)

readme_path = Path("./README.md").resolve()
readme = FileResource(
    uri=f"file://{readme_path.as_posix()}",
    path=readme_path,
    name="README File",
    description="The project's README.",
    mime_type="text/markdown",
    tags={"documentation"},
)
mcp.add_resource(readme)
```

The value case for `add_resource(...)` is deterministic assembly: plugin systems, mounted file/URL resources, programmatic registration from config, and bound-method/resource-library composition all become easier when registration is not coupled to inline decorator use. ([FastMCP][2])

### 7.4 Standalone `@resource(...)` for methods and reusable callables

FastMCP also exposes a standalone decorator in `fastmcp.resources.function_resource`: `resource(uri: str) -> Callable[[F], F]`. Its SDK page says it returns the original function with metadata attached and is meant to be registered later via `mcp.add_resource()`. The prose guide explicitly recommends this pattern for instance or class methods, mirroring the tool-method pattern. ([FastMCP][3])

```python id="55d6lg"
from fastmcp import FastMCP
from fastmcp.resources import resource

class RepoReader:
    def __init__(self, org: str):
        self.org = org

    @resource("repo://{name}/info")
    def get_repo(self, name: str) -> str:
        return f"{self.org}/{name}"

reader = RepoReader("PrefectHQ")
mcp = FastMCP("RepoServer")
mcp.add_resource(reader.get_repo)
```

This pattern matters because resource registration should see the **bound** signature, not `self` or `cls` as client-supplied URI/function parameters. ([FastMCP][3])

### 7.5 Return contract for resources

The prose docs define the conservative public contract as: resource functions should return `str`, `bytes`, or `ResourceResult`. `str` is emitted as text content with default `text/plain`; `bytes` becomes blob/binary content and should usually have an explicit MIME type; `ResourceResult` gives full control over multi-item content and metadata. The same guide advises that if you want to expose structured data like dicts or lists, you should explicitly `json.dumps()` them rather than returning raw objects, because that catches schema/content mistakes earlier in development. ([FastMCP][1])

There is one implementation nuance worth documenting explicitly for agents: the `FunctionResource` SDK reference says that the wrapped function can return `str`, `bytes`, or “other types,” with other types converted to JSON. That is slightly more permissive than the prose guide. The safest authoring rule is therefore: **treat explicit `str`/`bytes`/`ResourceResult` as the documented public contract, and use explicit JSON serialization for dict/list returns unless you intentionally want to rely on the SDK’s JSON-conversion path**. ([FastMCP][3])

### 7.6 `ResourceResult` and `ResourceContent`

`ResourceResult` is the explicit response envelope for resources. The docs define `contents` as `str | bytes | list[ResourceContent]`, with strings/bytes automatically wrapped into single content items, and `meta` as result-level metadata emitted into the MCP response’s `_meta` field. `ResourceContent` carries `content`, optional `mime_type`, and optional item-level `meta`; non-string/non-bytes values placed in `ResourceContent.content` are JSON-serialized automatically. ([FastMCP][1])

```python id="d1g2zd"
from fastmcp import FastMCP
from fastmcp.resources import ResourceResult, ResourceContent

mcp = FastMCP("RichResources")

@mcp.resource("data://users")
def get_users() -> ResourceResult:
    return ResourceResult(
        contents=[
            ResourceContent(content='[{"id": 1}]', mime_type="application/json"),
            ResourceContent(content="# Users\n...", mime_type="text/markdown"),
        ],
        meta={"total": 1},
    )
```

Use `ResourceResult` when one resource read should emit multiple content variants, custom MIME typing, CSP/caching hints, or other per-response metadata. Use plain `str`/`bytes` when the resource is single-payload and format-simple. ([FastMCP][1])

### 7.7 MCP `Context` inside resources

Resources and templates can request `Context` by adding a parameter annotated as `Context`. The resources guide shows both fixed-URI and template resources using `ctx: Context` to access request information, and the server SDK states that a `Context` parameter is optional and provides access to MCP features like logging, progress reporting, and session information. This parameter is runtime infrastructure, not client-supplied URI data. ([FastMCP][1])

```python id="h4h92s"
import json
from fastmcp import FastMCP, Context

mcp = FastMCP("ContextResources")

@mcp.resource("resource://system-status")
async def get_system_status(ctx: Context) -> str:
    return json.dumps({"status": "operational", "request_id": ctx.request_id})
```

### 7.8 Async resources and sync resource execution

FastMCP supports both `async def` and `def` resource functions. The resources guide states that synchronous resource functions are automatically run in a threadpool to avoid blocking the event loop, while async functions are the better fit for I/O-bound work such as async file reads, DB/network calls, or other awaitable pipelines. ([FastMCP][1])

```python id="jlwmco"
import aiofiles
from fastmcp import FastMCP

mcp = FastMCP("AsyncResources")

@mcp.resource("file:///app/data/important_log.txt", mime_type="text/plain")
async def read_important_log() -> str:
    async with aiofiles.open("/app/data/important_log.txt", mode="r") as f:
        return await f.read()
```

### 7.9 Direct resource classes

The direct concrete resource classes documented by FastMCP are `TextResource`, `BinaryResource`, `FileResource`, `HttpResource`, and `DirectoryResource`; the resource guide also notes `FunctionResource` as the internal class used by `@mcp.resource`. This split is important: `FunctionResource` is the lazy function wrapper; the others are concrete source-specific primitives. ([FastMCP][1])

#### `TextResource`

`TextResource` is the simplest concrete resource: string content already known at registration time. The SDK describes it as “a resource that reads from a string.” Use it for notices, banners, embedded help text, generated-but-static manifests, and similar immutable or rarely changing text payloads. ([FastMCP][4])

```python id="otvc6f"
from fastmcp.resources import TextResource

notice = TextResource(
    uri="resource://notice",
    name="Important Notice",
    text="System maintenance scheduled for Sunday.",
    tags={"notification"},
)
```

#### `BinaryResource`

`BinaryResource` is the bytes analogue of `TextResource`. The SDK describes it as a resource that reads from bytes. Use it when the payload is already in memory and should not be re-read from disk or fetched over HTTP. Explicit MIME typing is advisable for any non-generic binary content. ([FastMCP][4])

```python id="e5ybo3"
from fastmcp.resources import BinaryResource

logo = BinaryResource(
    uri="resource://logo",
    name="Logo",
    data=b"...binary bytes...",
    mime_type="image/png",
)
```

#### `FileResource`

`FileResource` reads from a local file path. The SDK says it enforces an **absolute** path, supports `is_binary=True` for binary reads, and can infer binary mode from MIME type if `is_binary` is not set explicitly; the prose guide adds that it handles text/binary modes, encoding, and lazy reading. Use it when the resource should be backed by an actual file rather than by a function wrapper. ([FastMCP][4])

```python id="j3npe2"
from pathlib import Path
from fastmcp.resources import FileResource

readme_path = Path("./README.md").resolve()

readme = FileResource(
    uri=f"file://{readme_path.as_posix()}",
    path=readme_path,
    name="README File",
    description="The project's README.",
    mime_type="text/markdown",
    tags={"documentation"},
)
```

#### `HttpResource`

`HttpResource` is the URL-backed direct resource. The SDK describes it as “a resource that reads from an HTTP endpoint,” and the prose guide notes that it requires `httpx`. Use it when the external HTTP content should be modeled as a readable MCP resource, not as an imperative tool call. ([FastMCP][4])

```python id="jlwmzu"
from fastmcp.resources import HttpResource

remote_schema = HttpResource(
    uri="https://example.com/schema.json",
    name="Remote Schema",
    description="Schema fetched from an external HTTP endpoint.",
)
```

#### `DirectoryResource`

`DirectoryResource` lists files in a directory. The SDK says it enforces an absolute path and provides `list_files()` plus `read()`; the resource guide’s example shows `recursive=False` and states that reads return a JSON list of files. Use it when discoverability of directory contents is itself the resource payload. ([FastMCP][4])

```python id="tzr5yd"
from pathlib import Path
from fastmcp.resources import DirectoryResource

data_dir = Path("./app_data").resolve()

listing = DirectoryResource(
    uri="resource://data-files",
    path=data_dir,
    name="Data Directory Listing",
    description="Lists files available in the data directory.",
    recursive=False,
)
```

### 7.10 Notifications

FastMCP automatically emits `notifications/resources/list_changed` when resources or templates are added, enabled, or disabled. The docs also give the critical caveat: notifications are only sent if those operations happen inside an active MCP request context, such as inside a tool or other MCP operation. Changes during server initialization do **not** emit notifications. This is a runtime/UI synchronization feature, not a startup manifest signal. ([FastMCP][1])

Operationally, this means dynamic resource topology changes performed in response to client activity can drive reactive UIs or auto-refreshing clients, but boot-time registration should still be treated as static startup state. ([FastMCP][1])

### 7.11 Annotations

Resources support `annotations=` on `@mcp.resource(...)`. The documented standard annotation keys are `readOnlyHint` and `idempotentHint`, both advisory hints intended to improve client behavior and UI without consuming prompt tokens. The docs explicitly say these hints are **advisory**, not enforcement mechanisms. ([FastMCP][1])

```python id="5plkdf"
@mcp.resource(
    "data://config",
    annotations={
        "readOnlyHint": True,
        "idempotentHint": True,
    },
)
def get_config() -> str:
    return '{"version": "1.0", "debug": false}'
```

For LLM-agent authors, the practical rule is: keep annotations semantically honest. Marking a resource `readOnlyHint=True` or `idempotentHint=True` helps clients optimize caching, display, and invocation UX, but it does not create a security or correctness boundary. ([FastMCP][1])

### 7.12 Resource templates: declaration and classification

Resource templates are created with the same `@mcp.resource(...)` decorator, but the URI contains placeholders like `{city}` or `{owner}/{repo}` and the backing function signature includes corresponding parameters. The docs say templates share most regular resource options—`name`, `description`, `mime_type`, `tags`, `annotations`, `meta`—but add URI parameter mapping and generate a concrete resource for each unique parameter set on demand. The server SDK also says that if the URI contains parameters **or** the function has parameters, the registration is treated as a template resource. ([FastMCP][1])

```python id="g1h9mw"
import json
from fastmcp import FastMCP

mcp = FastMCP("TemplateServer")

@mcp.resource("weather://{city}/current")
def get_weather(city: str) -> str:
    return json.dumps({
        "city": city.capitalize(),
        "temperature": 22,
        "condition": "Sunny",
    })
```

The docs also call out one signature asymmetry relative to tools: `*args` are not supported for resource templates, but `**kwargs` **are** supported because the URI template defines the exact parameter names that will be collected and passed as keyword arguments. That makes templates more flexible than tools in one narrow but important dimension. ([FastMCP][1])

### 7.13 RFC 6570 support: simple params, wildcards, query params

FastMCP implements RFC 6570 URI templates for resources, specifically supporting simple path parameters `{var}`, wildcard path parameters `{var*}`, and form-style query parameters `{?var1,var2}`. The SDK helper functions make that explicit: `extract_query_params(...)`, `build_regex(...)`, and `match_uri_template(...)` all operate on this subset. ([FastMCP][1])

#### Simple path parameters: `{var}`

A standard parameter matches exactly one URI path segment and does not cross `/` boundaries. Use it for stable one-segment identifiers like city names, IDs, or top-level keys. ([FastMCP][1])

#### Wildcard parameters: `{var*}`

Wildcard parameters capture multiple path segments, including slashes. The docs state they capture all subsequent path segments until the next literal/template boundary and that multiple wildcard parameters can exist in one template. This makes them appropriate for file paths, nested keys, repository paths, or REST-like hierarchical identifiers. ([FastMCP][1])

```python id="n44sk2"
@mcp.resource("files://{filename}")
def get_file(filename: str) -> str:
    return f"single segment: {filename}"

@mcp.resource("path://{filepath*}")
def get_path_content(filepath: str) -> str:
    return f"multi segment: {filepath}"

@mcp.resource("repo://{owner}/{path*}/template.py")
def get_template_file(owner: str, path: str) -> dict:
    return {"owner": owner, "path": path + "/template.py"}
```

#### Query parameters: `{?param1,param2}`

FastMCP supports RFC 6570 form-style query parameters via `{?param1,param2}`. The docs are strict here: query parameters must map to **optional** function parameters with default values, while path parameters map to required parameters. This is the clean separation rule: required data belongs in the path; optional configuration belongs in the query string. ([FastMCP][1])

```python id="fby7kx"
@mcp.resource("data://{id}{?format}")
def get_data(id: str, format: str = "json") -> str:
    if format == "xml":
        return f"<data id='{id}' />"
    return f'{{"id": "{id}"}}'

@mcp.resource("api://{endpoint}{?version,limit,offset}")
def call_api(endpoint: str, version: int = 1, limit: int = 10, offset: int = 0) -> dict:
    return {
        "endpoint": endpoint,
        "version": version,
        "limit": limit,
        "offset": offset,
    }
```

### 7.14 Type coercion for template/query parameters

The resource-template docs state that query-parameter values are automatically coerced from strings to the annotated parameter types based on the function signature, explicitly calling out `int`, `float`, `bool`, and `str`. This is a resource-side analogue of tool input coercion, but scoped to URI-extracted parameters instead of JSON request bodies. ([FastMCP][1])

A high-value corollary is also documented: optional parameters omitted from the URI template are **hidden defaults**. If a parameter has a default but is not included in `{?query}` or another path slot, clients cannot override it and the function always uses the default value. This is the cleanest way to keep runtime knobs internal without reaching for a separate dependency-injection mechanism. ([FastMCP][1])

### 7.15 Template validation rules

FastMCP enforces three template-creation rules:

1. required function parameters (no defaults) must appear in the URI **path** template;
2. query parameters declared via `{?param}` must be optional function parameters with defaults;
3. every URI-template parameter, whether path or query, must exist in the function signature. ([FastMCP][1])

The same section also documents what optional parameters may do:

* appear as query parameters, exposing them to clients;
* be omitted from the template, making them fixed internal defaults;
* be used in alternative path templates, allowing multiple URI patterns for the same function. ([FastMCP][1])

That third case is particularly useful for compatibility layers or dual-lookup resources. The docs show manually applying multiple decorators to one function, e.g. `users://email/{email}` and `users://name/{name}` bound to a single implementation. ([FastMCP][1])

### 7.16 Matching internals and validation helpers

For custom providers, testing, or debugging, the SDK exposes three resource-template helper functions:

* `extract_query_params(uri_template: str) -> set[str]`
* `build_regex(template: str) -> re.Pattern[str] | None`
* `match_uri_template(uri: str, uri_template: str) -> dict[str, str] | None` ([FastMCP][5])

`build_regex(...)` understands `{var}`, `{var*}`, and `{?var1,var2}`, ignoring query parameters during path matching. It returns `None` when the resulting regex would be invalid—for example, when parameter names contain hyphens, start with digits, or create duplicates coming from a remote server. `match_uri_template(...)` then performs the full URI match and extracts both path and query parameters. This is the implementation-level contract behind template resolution, and it is the correct primitive when writing custom template-aware providers or test harnesses. ([FastMCP][5])

### 7.17 Best-practice deployment and authoring guidance

Prefer a **resource** over a **tool** when the capability is read-only, addressable by URI, and semantically cacheable/idempotent; prefer a **tool** when the operation is imperative, side-effecting, conversationally parameterized, or not naturally modeled as “read this URI.” FastMCP’s own docs frame resources as read-only data/file access and tools as executable capabilities, so that split should be encoded into agent design heuristics. ([FastMCP][1])

Prefer `@mcp.resource(...)` for function-backed dynamic content and use concrete classes like `TextResource`, `FileResource`, `HttpResource`, or `DirectoryResource` when the content source is already a stable object/file/URL and does not need a dedicated function body. The resource guide explicitly presents that as the division of labor. ([FastMCP][1])

Prefer explicit `mime_type=` for any binary or non-obvious textual content. The docs repeatedly note that MIME type can be inferred, but they also recommend explicit values for non-text or specialized content; `FileResource` and `BinaryResource` in particular benefit from explicit typing. ([FastMCP][1])

Use absolute filesystem paths for `FileResource` and `DirectoryResource`. The SDK pages enforce absolute-path validation, so relative-path registration should be normalized with `.resolve()` at build time rather than left ambiguous. ([FastMCP][4])

For template design, keep required identifiers in path segments and optional knobs in `{?query}` parameters; omit optional parameters from the URI template entirely when they should remain hidden defaults. The docs define exactly this separation and even contrast configurable-via-query and fixed-default examples. ([FastMCP][1])

If a client UI needs to react to runtime resource topology changes, perform add/enable/disable operations inside an active MCP request context so `notifications/resources/list_changed` will be emitted. Initialization-time registration will not notify clients. ([FastMCP][1])


[1]: https://gofastmcp.com/servers/resources "Resources & Templates - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-server-server "server - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-resources-function_resource "function_resource - FastMCP"
[4]: https://gofastmcp.com/python-sdk/fastmcp-resources-types "types - FastMCP"
[5]: https://gofastmcp.com/python-sdk/fastmcp-resources-template "template - FastMCP"

# FastMCP Advanced — 8) Prompts and prompt rendering
### Prompts and prompt rendering

### 8.0 Prompt model: definition-time contract vs render-time contract

Prompts are reusable, parameterized message templates. When a client requests a prompt, FastMCP locates the prompt definition, validates any arguments against the function signature, executes the function, and returns the generated message sequence to the LLM. By default, the prompt name comes from the Python function name and the prompt description comes from the function docstring. Functions with `*args` or `**kwargs` are not supported because FastMCP must generate a complete MCP parameter schema for the prompt surface. ([FastMCP][1])

For agent authors, the operational split is: **definition-time metadata** controls what shows up in `prompts/list`; **render-time output** controls the concrete message sequence returned by `get_prompt`/`render_prompt`. Those are related but not the same object. `FastMCP.get_prompt(...)` returns a prompt definition filtered through visibility/version rules, while `FastMCP.render_prompt(...)` executes the prompt and returns a `PromptResult` (or a task handle when backgrounded). ([FastMCP][2])

### 8.1 `@mcp.prompt` — direct registration surface

The primary registration surface is `@mcp.prompt`. The server SDK documents the supported calling patterns as `@server.prompt`, `@server.prompt()`, `@server.prompt("custom_name")`, `@server.prompt(name="custom_name")`, and direct function registration `server.prompt(function, name="custom_name")`. The prompt function may also request `Context` by type annotation. ([FastMCP][2])

Minimal direct-registration form:

```python id="5n6qk7"
from fastmcp import FastMCP
from fastmcp.prompts import Message

mcp = FastMCP(name="PromptServer")

@mcp.prompt
def ask_about_topic(topic: str) -> str:
    """Generates a user message asking for an explanation of a topic."""
    return f"Can you please explain the concept of '{topic}'?"

@mcp.prompt
def generate_code_request(language: str, task_description: str) -> list[Message]:
    """Generates a conversation for code generation."""
    return [
        Message(f"Write a {language} function that performs the following task: {task_description}"),
        Message("I'll help you write that function.", role="assistant"),
    ]
```

That shape is the canonical MCP prompt declaration pattern: prompt identifier from function name unless overridden, parameter schema from the signature, and render contract from the return type. ([FastMCP][1])

### 8.2 `mcp.add_prompt(...)` and standalone `@prompt()` for methods

`FastMCP.add_prompt(...)` is the programmatic registration API. The server SDK defines it as accepting either a `Prompt` instance or an `@prompt`-decorated function and returning the registered `Prompt`. This is the correct entrypoint for composition, plugin discovery, bound-method registration, and server assembly from external component libraries. ([FastMCP][2])

The standalone decorator lives at `fastmcp.prompts.function_prompt.prompt(...)`. Its SDK page says it marks a function as an MCP prompt, returns the original function with metadata attached, and is meant to be registered later via `mcp.add_prompt()`. This is the correct pattern for instance/class methods, because immediate `@mcp.prompt` registration would see `self`/`cls` at decoration time instead of the later bound method signature. ([FastMCP][3])

```python id="72t62r"
from fastmcp import FastMCP
from fastmcp.prompts import Message
from fastmcp.prompts import prompt

class Reviewer:
    def __init__(self, style: str):
        self.style = style

    @prompt()
    def review_request(self, code: str) -> list[Message]:
        return [
            Message(f"Review this code in {self.style} style:\n\n{code}"),
            Message("I will review the code now.", role="assistant"),
        ]

reviewer = Reviewer(style="security-focused")

mcp = FastMCP("ReviewServer")
mcp.add_prompt(reviewer.review_request)
```

For method-based prompts, that separation is the safe contract: standalone `@prompt()` attaches metadata; `mcp.add_prompt(bound_method)` publishes the correct bound signature. ([FastMCP][3])

### 8.3 Prompt decorator arguments: public docs surface and current SDK surface

The main prompts guide documents these decorator arguments: `name`, `title`, `description`, `tags`, deprecated `enabled`, `icons`, `meta`, and `version`. Semantically: `name` overrides the exposed MCP prompt name, `title` is a human-readable title, `description` overrides the docstring-derived description, `tags` categorize prompts, `icons` attach presentation metadata, `meta` is static prompt-definition metadata surfaced to clients, and `version` is the dedicated component-version field introduced in `3.0.0`. The guide explicitly marks `enabled` deprecated in v3 and directs users to server-level `mcp.enable()` / `mcp.disable()` instead. ([FastMCP][1])

The lower-level prompt decorator reference for `LocalProvider` lists a slightly broader current surface: in addition to `name`, `title`, `description`, `icons`, `tags`, deprecated `enabled`, and `meta`, it also lists `task` and `auth` as supported decorator kwargs. For agent-authored docs/code, the safest framing is: the prose guide highlights the common user-facing fields, while the SDK reference exposes the broader current decorator surface used by the runtime. ([FastMCP][4])

Representative explicit-decorator form:

```python id="s83kzd"
from fastmcp import FastMCP

mcp = FastMCP("PromptServer")

@mcp.prompt(
    name="analyze_data_request",
    title="Analyze Data Request",
    description="Creates a request to analyze data with specific parameters.",
    tags={"analysis", "data"},
    meta={"owner": "data-team", "stability": "ga"},
    version="2.0",
)
def data_analysis_prompt(data_uri: str, analysis_type: str = "summary") -> str:
    return f"Please perform a '{analysis_type}' analysis on the data found at {data_uri}."
```

If `description=` is supplied, the function docstring is ignored for prompt-description purposes. If it is omitted, the docstring becomes the prompt’s description in MCP listings. ([FastMCP][1])

### 8.4 Typed prompt arguments: string-on-the-wire, typed-in-Python

Prompt arguments have a special transport rule: MCP prompt arguments are passed as **strings** at the protocol layer, but FastMCP lets you annotate the Python function with richer types and performs server-side conversion. The prompts guide explicitly documents this for complex types like `list[int]` and `dict[str, str]`: FastMCP converts incoming strings to the annotated Python types, augments prompt argument descriptions with JSON-schema-like formatting guidance, and still lets you call the prompt directly in Python with properly typed values. ([FastMCP][1])

```python id="l8rnyr"
from fastmcp import FastMCP

mcp = FastMCP("TypedPrompts")

@mcp.prompt
def analyze_data(
    numbers: list[int],
    metadata: dict[str, str],
    threshold: float,
) -> str:
    avg = sum(numbers) / len(numbers)
    return f"Average: {avg}, above threshold: {avg > threshold}"
```

At the protocol layer, a client sends:

```json id="1xtzyv"
{
  "numbers": "[1, 2, 3, 4, 5]",
  "metadata": "{\"source\": \"api\", \"version\": \"1.0\"}",
  "threshold": "2.5"
}
```

But direct Python-side rendering can still use typed values. The docs also warn to keep prompt argument types simple: `list[int]`, `dict[str, str]`, `float`, and `bool` are good candidates; deeply nested custom structures or complex Pydantic models may not convert reliably from JSON strings because the only guidance clients/LLMs receive is the generated argument description. ([FastMCP][1])

Required-vs-optional behavior follows normal Python signature rules: parameters without defaults are required; parameters with defaults are optional. FastMCP uses that distinction when exposing prompt arguments through MCP. ([FastMCP][1])

### 8.5 No `*args` / `**kwargs`: prompt schema must be total

FastMCP explicitly forbids `*args` and `**kwargs` in prompt functions. The reason is the same as for tools but even sharper here because prompt arguments are listed in MCP as named prompt parameters: FastMCP must emit a complete parameter schema, and variable argument lists cannot be represented as a stable, machine-readable contract for clients. ([FastMCP][1])

For agent-authored code, the design rule is: expose a fixed prompt schema with explicit parameter names and defaults. If internal logic wants variadic flexibility, hide it behind a stable wrapper prompt function rather than attempting to surface a variadic Python signature directly. ([FastMCP][1])

### 8.6 Return contract: `str`, `list[Message | str]`, or `PromptResult`

The prompt return contract is intentionally narrow. The docs define the only supported prompt-function return shapes as:

* `str`: sent as a single user message
* `list[Message | str]`: a conversation/message sequence; strings are auto-converted to user messages
* `PromptResult`: full control over messages, description, and runtime metadata. ([FastMCP][1])

That same contract is repeated in the prompt base SDK: `Prompt.from_function(...)` and `Prompt.render(...)` both describe those three accepted return classes, and `Prompt.convert_result(...)` is the adapter that normalizes them into `PromptResult`, raising `TypeError` for unsupported return types. ([FastMCP][5])

Minimal forms:

```python id="p44jih"
from fastmcp import FastMCP
from fastmcp.prompts import Message, PromptResult

mcp = FastMCP("PromptServer")

@mcp.prompt
def simple(topic: str) -> str:
    return f"Explain {topic}"

@mcp.prompt
def dialogue(topic: str) -> list[Message | str]:
    return [
        f"Explain {topic}",
        Message("I can help with that.", role="assistant"),
    ]

@mcp.prompt
def rich(topic: str) -> PromptResult:
    return PromptResult(
        messages=[
            Message(f"Explain {topic}"),
            Message("I will structure the explanation.", role="assistant"),
        ],
        description="Explanation prompt",
        meta={"category": "education"},
    )
```

This is the full public render contract. Anything else must be explicitly transformed into one of these forms before returning. ([FastMCP][1])

### 8.7 `Message` — canonical prompt-message primitive

In v3-style FastMCP, the canonical prompt message type is `fastmcp.prompts.Message`, not `mcp.types.PromptMessage`. The upgrade guide is explicit: prompt functions now use `Message`, which defaults `role="user"` and accepts plain strings directly; the same guide also says raw dicts with `role`/`content` keys were silently coerced in v2 but should be replaced in v3 with `Message` objects or plain strings for single-user messages. ([GitHub][6])

The prompt base SDK and the prompts guide define `Message` as a wrapper with two fields:

* `content`: any content; strings pass through, while dict/list/BaseModel values are JSON-serialized to text
* `role`: `"user"` or `"assistant"`, default `"user"`. ([FastMCP][1])

```python id="ty6r3f"
from fastmcp.prompts import Message

Message("Hello")                              # user role
Message("I can help.", role="assistant")      # assistant role
Message({"task": "summarize"})                # JSON-serialized to text
```

The important distinction is: `Message(content=<dict>)` is valid because the dict becomes text content inside a typed message wrapper; returning a raw dict in place of a `Message` object is no longer the intended v3 contract. ([GitHub][6])

### 8.8 `PromptResult` — render-time control plane

`PromptResult` is the canonical explicit result type for prompt rendering. The prompts guide defines its fields as `messages`, `description`, and `meta`. `messages` can be a string or a list of `Message`; strings are wrapped as a single user message. `description` is optional and, if omitted, defaults to the prompt’s docstring. `meta` is result-level runtime metadata and is placed into the MCP response’s `_meta` field. ([FastMCP][1])

```python id="fyh8zs"
from fastmcp.prompts import PromptResult, Message

PromptResult(
    messages=[
        Message("Please review this code."),
        Message("I will check correctness, style, and security.", role="assistant"),
    ],
    description="Code review prompt",
    meta={"review_type": "security", "priority": "high"},
)
```

The base SDK also describes `PromptResult` as the explicit-control render type and exposes `to_mcp_prompt_result()` for conversion into the wire-level MCP result object. For agent authors, treat `PromptResult` as the “escape hatch” when plain `str` or `list[Message]` is insufficient because you need per-render description or per-render metadata. ([FastMCP][5])

### 8.9 Static prompt metadata vs runtime prompt-result metadata

FastMCP has two separate metadata planes for prompts, and confusing them produces poor client behavior.

* `@mcp.prompt(meta={...})` is **static definition metadata**. The prompts guide says this is passed through to the client-side prompt object’s `meta` field and can be used for custom metadata, versioning hints, or other application-specific prompt catalog information. This is what clients see when listing prompts. ([FastMCP][1])
* `PromptResult(meta={...})` is **runtime render metadata**. The prompts guide says this is included in the MCP response’s `_meta` field and is meant for per-render metadata such as categorization, priority, or other client-specific runtime data. It explicitly distinguishes this from decorator-level `meta`. ([FastMCP][1])

That means the correct split is:

* stable catalog metadata, ownership, UI hints, rollout labels, etc. → decorator `meta=...`
* per-render classification, priority, execution markers, dynamic tags, etc. → `PromptResult.meta` ([FastMCP][1])

Likewise, static `description=` on the decorator describes the prompt definition exposed via MCP listing, while `PromptResult.description` describes a specific render result. They live at different layers of the protocol. ([FastMCP][1])

### 8.10 Async prompts and `Context`

FastMCP supports both synchronous and asynchronous prompt functions. The prompts guide states that `def` prompts are automatically run in a threadpool to avoid blocking the event loop, while `async def` is preferable for I/O-bound work such as database or network access. ([FastMCP][1])

Prompts can also request `Context` by type annotation. The prompts guide says adding a `Context` parameter gives access to request-local MCP information and features such as request IDs and other context utilities. As with tools/resources, `Context` is a runtime-injected infrastructure parameter, not a client-supplied prompt argument. ([FastMCP][1])

```python id="rbukdk"
from fastmcp import FastMCP, Context

mcp = FastMCP(name="PromptServer")

@mcp.prompt
async def generate_report_request(report_type: str, ctx: Context) -> str:
    return f"Please create a {report_type} report. Request ID: {ctx.request_id}"
```

### 8.11 Rendering APIs: definition lookup vs execution

At the server API layer, `get_prompt(name, version=None)` retrieves the prompt definition without rendering it; `render_prompt(name, arguments=None, version=None, ...)` performs the actual render and returns a `PromptResult` by default. The server SDK documents both, and also notes that middleware is applied by default when rendering. ([FastMCP][2])

The current SDK also exposes a background-execution pathway for prompts. `render_prompt(...)` can return `PromptResult | CreateTaskResult` depending on whether `task_meta` is supplied, and the local-provider prompt decorator reference lists `task` as an optional decorator configuration field for background execution. For most prompt workloads this is unnecessary, but it is part of the current prompt object model. ([FastMCP][2])

### 8.12 Versioning: multiple implementations, one public name

Prompt versioning is a first-class feature in v3. The prompts guide says prompts support versioning, allowing you to maintain multiple implementations under the same name while clients automatically receive the highest version by default. The decorator-level `version=` field is the dedicated version identifier for this purpose. ([FastMCP][1])

The server SDK gives the operational lookup rules:

* `get_prompt(name, version=None)` returns the highest enabled version when no explicit version is requested
* if the highest version is disabled and no explicit version is requested, FastMCP falls back to the next-highest enabled version
* `render_prompt(name, ..., version=...)` can target a specific version instead of the default highest version. ([FastMCP][2])

Representative registration pattern:

```python id="jlwmog"
from fastmcp import FastMCP

mcp = FastMCP("VersionedPrompts")

@mcp.prompt(name="summarize", version="1.0")
def summarize_v1(text: str) -> str:
    return f"Summarize this text:\n\n{text}"

@mcp.prompt(name="summarize", version="2.0")
def summarize_v2(text: str, style: str = "concise") -> str:
    return f"Summarize this text in a {style} style:\n\n{text}"
```

With that definition set, default prompt retrieval/rendering uses `2.0`, while explicit version selection can target `1.0`. This is the correct migration pattern when you need to evolve a prompt contract without breaking existing clients or workflows. ([FastMCP][1])

Version comparison itself is not arbitrary string sorting. The version-utility SDK says versions are first parsed as PEP 440 where possible, falling back to lexicographic comparison for invalid/non-PEP-440 strings; `None` sorts lowest. Examples given by the SDK include `"1" < "2" < "10"` semantically, `"v1.0"` normalized to `"1.0"`, and date-like strings compared lexicographically if they are not valid PEP 440 versions. ([FastMCP][7])

For agent authors, the design implication is direct: use PEP-440-like version strings (`"1.0"`, `"2.1"`, `"3.0rc1"`) if you want predictable semantic ordering. Non-PEP-440 strings still work, but then ordering degrades to lexicographic behavior. ([FastMCP][7])

### 8.13 Best-practice prompt authoring rules for agents

Prefer `@mcp.prompt` for top-level prompt functions and standalone `@prompt()` plus `mcp.add_prompt(bound_method)` for methods. That preserves correct bound signatures and keeps `self`/`cls` out of the public prompt schema. ([FastMCP][3])

Keep prompt argument types simple and string-serializable from MCP clients. The prompt docs explicitly recommend simple lists/dicts/scalars and warn against complex nested/custom classes because prompt arguments arrive as strings and conversion guidance is description-driven. ([FastMCP][1])

Use `Message` and `PromptResult`, not legacy `mcp.types.PromptMessage` or raw dict prompt-message payloads. The upgrade guide makes that the v3 contract, and the prompt/message docs define the modern primitives clearly. ([GitHub][6])

Use decorator `meta=` for static catalog metadata and `PromptResult.meta` for per-render runtime metadata; do not mix the two planes. Use `version=` for actual component selection/version routing rather than stuffing version semantics into arbitrary `meta` fields. ([FastMCP][1])

Use versioning to evolve prompts under a stable public name instead of renaming prompts gratuitously. FastMCP already handles highest-version selection and explicit version targeting; stable names plus explicit versions make both human and machine clients easier to maintain. ([FastMCP][1])


[1]: https://gofastmcp.com/servers/prompts "Prompts - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-server-server "server - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-prompts-function_prompt "function_prompt - FastMCP"
[4]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-local_provider-decorators-prompts "prompts - FastMCP"
[5]: https://gofastmcp.com/python-sdk/fastmcp-prompts-base "base - FastMCP"
[6]: https://github.com/PrefectHQ/fastmcp/blob/main/docs/getting-started/upgrading/from-fastmcp-2.mdx "fastmcp/docs/getting-started/upgrading/from-fastmcp-2.mdx at main · PrefectHQ/fastmcp · GitHub"
[7]: https://gofastmcp.com/python-sdk/fastmcp-utilities-versions "versions - FastMCP"

# FastMCP Advanced — 9) MCP Context
### MCP Context

### Version anchors

`Context.sample()` is documented as available from `2.0.0`; `get_context()` from `2.2.11`; `ctx.elicit()` from `2.10.0`; `ctx.list_resources()`, `ctx.list_prompts()`, and `ctx.get_prompt()` from `2.13.0`; `ctx.request_context.meta` and request-context availability semantics from `2.13.1`; `CurrentContext()` as the preferred explicit injection style from `2.14`; session state and per-session visibility methods from `3.0.0`; and current docs also note that `Context` now works transparently in Docket background workers, with task-aware elicitation/sampling behavior. ([FastMCP][1])

### 9.0 Role: what `Context` is and what it is not

`Context` is FastMCP’s request-scoped gateway to MCP runtime capabilities. The official context docs define it as the interface for logging, progress reporting, resource access, prompt access, LLM sampling, user elicitation, session state, session visibility, request metadata, and underlying server access from within tools, resources, templates, and prompts. It is therefore an **integration boundary object**, not a domain-data parameter. Use it when code must talk to the MCP runtime; do not use it as a generic substitute for explicit business inputs. ([FastMCP][1])

The boundary is sharp: plain function parameters are for caller-supplied business data; `Context` is for server/session/request/protocol behavior. If a function only needs `customer_id`, `amount`, `filters`, or `path`, use plain typed parameters. If it needs to emit client logs, read another registered resource, render a prompt, store session state, elicit missing data, sample through the client’s LLM, or inspect request/session metadata, use `Context`. FastMCP’s own DI docs reinforce this separation by presenting `Context` alongside other runtime-only injected values like `CurrentRequest()`, `CurrentAccessToken()`, and `CurrentFastMCP()`. ([FastMCP][1])

### 9.1 Injection surface: three access patterns

The preferred modern injection form is explicit dependency injection with `CurrentContext()`. The context docs say this is the preferred approach since `2.14`, that dependency parameters are automatically excluded from the MCP schema, and that context methods are async, so the containing function is usually async as well. This works uniformly for tools, resources, templates, and prompts. ([FastMCP][1])

```python id="5900jg"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

mcp = FastMCP(name="Context Demo")

@mcp.tool
async def process_file(file_uri: str, ctx: Context = CurrentContext()) -> str:
    await ctx.info(f"Processing {file_uri}")
    return "Processed file"
```

Legacy type-hint injection still works: any parameter annotated as `Context` is injected automatically, regardless of the parameter name, and the docs explicitly say unions like `Context | None` or `Annotated[...]` forms also work. This is concise, but the dependency is less visible to readers and code generators. For new agent-authored code, `CurrentContext()` is usually the clearer signature. ([FastMCP][1])

```python id="x8t8uw"
from fastmcp import FastMCP, Context

mcp = FastMCP("Context Demo")

@mcp.tool
async def process_file(file_uri: str, ctx: Context) -> str:
    await ctx.info(f"Processing {file_uri}")
    return "Processed file"
```

For deeper helper layers where passing `ctx` through every call is undesirable, FastMCP provides `get_context()`. The context and DI docs say it retrieves the active context from anywhere within a request’s execution flow. This is the right escape hatch for logging/utilities/adapters inside a live request; it is the wrong primitive for general application architecture because it couples the callee to ambient request state. ([FastMCP][1])

```python id="sn1s1m"
from fastmcp.server.dependencies import get_context

async def log_step(message: str) -> None:
    ctx = get_context()
    await ctx.info(message)
```

### 9.2 Request-scope semantics

Each MCP request receives a new `Context` object. The docs are explicit that context is scoped to a single request and only available during request execution; attempting to use context methods outside a request raises errors. That means request-local data should live on the current `Context`, but request-to-request persistence belongs in session state or lifespan-managed infrastructure, not in the `Context` instance itself. ([FastMCP][1])

A useful corollary comes from the SDK context page: `ctx.request_context` may be `None` before the MCP session is established, particularly during early middleware or connection initialization, while some HTTP request data can still exist. Therefore “request exists” and “MCP session exists” are not identical states. Code that needs MCP session semantics must guard for `ctx.request_context is not None`; code that only needs HTTP headers/client IP should use the HTTP-request DI helpers instead. ([FastMCP][2])

### 9.3 Core properties and structural capabilities

The SDK context surface exposes the core structural properties that matter to agents: `fastmcp`, `request_context`, `lifespan_context`, `transport`, `client_supports_extension()`, `client_id`, `request_id`, `session_id`, `session`, `is_background_task`, `task_id`, and `origin_request_id`. These define the context’s role as the bridge between your function body and the active server/session/request/task environment. ([FastMCP][2])

High-value members:

* `ctx.fastmcp -> FastMCP`: server instance access.
* `ctx.request_context -> RequestContext | None`: MCP request/session object when available.
* `ctx.lifespan_context -> dict[str, Any]`: values yielded by the server lifespan.
* `ctx.transport -> Literal["stdio", "sse", "streamable-http"] | None`: runtime transport mode.
* `ctx.client_supports_extension(extension_id) -> bool`: capability check against client-advertised extensions.
* `ctx.client_id`, `ctx.request_id`, `ctx.session_id`, `ctx.session`: request/session identity and lower-level session access.
* `ctx.is_background_task()`, `ctx.task_id`, `ctx.origin_request_id`: task-mode awareness. ([FastMCP][2])

Representative introspection pattern:

```python id="42f5hn"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

mcp = FastMCP("Introspection")

@mcp.tool
async def request_info(ctx: Context = CurrentContext()) -> dict:
    return {
        "server": ctx.fastmcp.name,
        "transport": ctx.transport,
        "request_id": ctx.request_id,
        "client_id": ctx.client_id,
        "session_id": ctx.session_id,
        "background": ctx.is_background_task(),
        "task_id": ctx.task_id,
        "origin_request_id": ctx.origin_request_id,
    }
```

Use `ctx.fastmcp` and `ctx.lifespan_context` when you need server-owned resources or startup-managed shared objects; use plain parameters or custom dependencies for domain data. The SDK says `lifespan_context` returns the dict yielded by the server’s lifespan function and even falls back correctly in background-task contexts. That makes it the correct place to read long-lived infrastructure handles like DB pools or clients created at startup. ([FastMCP][2])

### 9.4 Logging: client-visible operational telemetry

`Context` exposes `log(message, level=None, logger_name=None, extra=None)` plus convenience helpers `debug()`, `info()`, `warning()`, and `error()`. The SDK says messages sent to clients are also logged to `fastmcp.server.context.to_client` at `DEBUG`, and the context guide positions these methods as the way to surface execution visibility back to the MCP client. This is client-facing operational telemetry, not ordinary server-internal Python logging. ([FastMCP][1])

```python id="jlwmck"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Logging")

@mcp.tool
async def analyze(data: list[int], ctx: Context = CurrentContext()) -> int:
    await ctx.debug("Starting analysis")
    await ctx.info(f"Processing {len(data)} items")
    if not data:
        await ctx.warning("No items supplied")
        return 0
    result = sum(data)
    if result < 0:
        await ctx.error("Unexpected negative sum")
    return result
```

Use `ctx.*log*` methods when the client/operator should see progress or diagnosis in-band over MCP. Use normal Python logging for internal service logs, background metrics, and infrastructure traces that are not part of the client conversation. That split follows directly from the SDK wording that these methods “send a message to the connected MCP Client.” ([FastMCP][2])

### 9.5 Progress reporting

`ctx.report_progress(progress, total=None, message=None)` emits MCP progress notifications in foreground mode and Docket task progress updates in background mode. The SDK explicitly says it works in both contexts; the context guide frames it as the standard way to support client progress indicators for long-running work. ([FastMCP][1])

```python id="h6r9vc"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Progress")

@mcp.tool
async def import_rows(rows: list[dict], ctx: Context = CurrentContext()) -> int:
    total = len(rows)
    for i, row in enumerate(rows, start=1):
        # process row...
        await ctx.report_progress(i, total, f"Imported {i}/{total}")
    return total
```

Use progress when the operation is materially longer than a typical request and the client can benefit from incremental visibility. Do not overuse it for near-instant operations; emitting progress for trivial work adds noise without improving UX. The docs position progress as a long-running-operation aid, not a default logging replacement. ([FastMCP][1])

### 9.6 Resource access from within tools/prompts/resources

`Context` can inspect and consume server resources from inside other MCP components. The context docs define `ctx.list_resources() -> list[MCPResource]` and `ctx.read_resource(uri) -> ResourceResult / list[ReadResourceContents]` depending on documentation layer, and the examples show listing resources and reading content by URI. This enables composition patterns such as “tool reads config resource,” “prompt pulls a data resource before rendering,” or “middleware inspects a resource-backed catalog.” ([FastMCP][1])

```python id="t2f0to"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("ResourceAccess")

@mcp.resource("resource://config")
def config() -> str:
    return '{"mode":"prod"}'

@mcp.tool
async def read_config(ctx: Context = CurrentContext()) -> str:
    resources = await ctx.list_resources()
    result = await ctx.read_resource("resource://config")
    return result.contents[0].content if hasattr(result, "contents") else result[0].content
```

Use `ctx.read_resource()` when the abstraction should remain a resource lookup. Do not bypass that abstraction by importing the backing Python function directly unless you explicitly want tight in-process coupling. The value of resource access is that it preserves the server’s own visibility, versioning, and resolution semantics. ([FastMCP][1])

### 9.7 Prompt access from within tools/middleware

`Context` also exposes prompt discovery and rendering helpers: `ctx.list_prompts()` and `ctx.get_prompt(name, arguments=None)`. The context guide says these were added in `2.13.0` and are meant to let tools and middleware discover and use registered prompts programmatically. ([FastMCP][1])

```python id="r5igyp"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("PromptAccess")

@mcp.prompt
def analyze_data(dataset: str) -> str:
    return f"Analyze dataset: {dataset}"

@mcp.tool
async def render_analysis_prompt(dataset: str, ctx: Context = CurrentContext()) -> list[str]:
    prompts = await ctx.list_prompts()
    result = await ctx.get_prompt("analyze_data", {"dataset": dataset})
    return [m.content.text if hasattr(m.content, "text") else str(m.content) for m in result.messages]
```

Use `ctx.get_prompt()` when you need the rendered prompt result as data or as part of a higher-order workflow. Do not use it as a substitute for ordinary Python function calls between internal modules unless prompt rendering semantics are actually desired. The MCP boundary adds value only when the prompt catalog itself is part of the server’s programmable surface. ([FastMCP][1])

### 9.8 Client elicitation

`ctx.elicit(...)` requests structured user input during an operation. The context guide says this enables interactive workflows and progressive disclosure; the SDK context page gives the overload surface and notes that the client must support elicitation or the request will error. It also states a protocol constraint: MCP elicitation only supports simple object schemas with primitive fields. FastMCP lets you provide a primitive type, dataclass, `TypedDict`, or `BaseModel`; primitive types are wrapped into a single `"value"` field for protocol compliance and then deconstructed back into the primitive on return. If `response_type=None`, the elicitation schema is an empty object and the client must respond with an empty object. ([FastMCP][1])

```python id="twwjlwm"
from dataclasses import dataclass
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Elicitation")

@dataclass
class Approval:
    approved: bool
    reason: str

@mcp.tool
async def confirm_delete(ctx: Context = CurrentContext()) -> str:
    result = await ctx.elicit("Confirm deletion", response_type=Approval)
    if result.action == "accept":
        return f"approved={result.data.approved} reason={result.data.reason}"
    return f"action={result.action}"
```

SDK overloads cover several response-shape families: primitive types, structured object types, option lists, titled options, multi-select lists, and `None`. The accepted result union is `AcceptedElicitation[...] | DeclinedElicitation | CancelledElicitation`, so robust code should always branch on `result.action` rather than assuming acceptance. ([FastMCP][2])

Use `ctx.elicit()` only when the operation genuinely needs user interaction mid-flight. It is the right primitive for missing parameters, clarifications, approvals, or progressive disclosure; it is the wrong primitive for values that should have been ordinary input arguments in the first place. The docs frame elicitation as an interactive exception path, not a replacement for normal schema design. ([FastMCP][1])

### 9.9 LLM sampling

`ctx.sample(...)` is the in-request LLM orchestration primitive. The context docs say it requests the client’s LLM to generate text; the dedicated sampling SDK page and context SDK clarify that `sample()` runs a full loop to completion, optionally executing tool calls until a final text response is produced, while `sample_step()` performs exactly one sampling step and optionally one tool-execution turn for fine-grained control. When `result_type` is provided, FastMCP synthesizes a `final_response` tool and validates the final structured answer against that type, returning it as `.result`. ([FastMCP][1])

Key `sample(...)` arguments documented in the SDK include `messages`, `system_prompt`, `temperature`, `max_tokens`, `model_preferences`, `tools`, `result_type`, `mask_error_details`, and `tool_concurrency`. `tool_concurrency=None` means sequential execution, `0` means unlimited parallelism, and a positive integer bounds concurrency; if any tool is marked `sequential=True`, tool execution is forced back to sequential behavior. The changelog/updates page also calls out concurrent sampling as a recent capability. ([FastMCP][2])

```python id="fcb2qi"
from dataclasses import dataclass
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Sampling")

@dataclass
class Summary:
    title: str
    score: int

@mcp.tool
async def summarize(text: str, ctx: Context = CurrentContext()) -> dict:
    result = await ctx.sample(
        messages=[f"Summarize this text: {text}"],
        temperature=0.2,
        result_type=Summary,
        tool_concurrency=None,
    )
    return {"text": result.text, "result": result.result}
```

Use `sample()` when a tool/resource/prompt needs model assistance inline and you want FastMCP to manage the tool loop. Use `sample_step()` when you want manual control over intermediate tool-use handling or conversation-state evolution. The SDK explicitly makes that distinction. ([FastMCP][2])

### 9.10 Session state

FastMCP `Context` provides session-scoped state that persists across requests within the same MCP session. The context guide says this is new in `3.0.0`, automatically keyed by client session, isolated between clients, and exposed through async methods `set_state`, `get_state`, and `delete_state`. State set in `on_initialize` middleware can persist into later calls when the same session object is reused, including STDIO, SSE, and single-server HTTP; in distributed/serverless HTTP deployments, isolation follows the `mcp-session-id` header. ([FastMCP][1])

```python id="z4o8v4"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Stateful")

@mcp.tool
async def increment(ctx: Context = CurrentContext()) -> int:
    count = await ctx.get_state("counter") or 0
    await ctx.set_state("counter", count + 1)
    return count + 1

@mcp.tool
async def reset(ctx: Context = CurrentContext()) -> None:
    await ctx.delete_state("counter")
```

The docs add several important operational rules. Session state values must normally be JSON-serializable so they can persist across requests. If you call `set_state(..., serializable=False)`, the value is stored only in a request-scoped dict for the current request and is not available to later requests. The SDK also says `get_state` checks request-scoped nonserializable values first, then session-scoped state. ([FastMCP][1])

```python id="xg8x36"
@mcp.tool
async def one_request_client(ctx: Context = CurrentContext()) -> str:
    client = SomeHttpClient()
    await ctx.set_state("client", client, serializable=False)
    same_request_client = await ctx.get_state("client")
    return str(same_request_client is client)
```

The context guide says the default session-state backend is in-memory and suitable for single-server deployments, and recommends a custom `session_state_store` backend such as Redis for distributed or serverless deployments. This is where deployment architecture matters directly: if session continuity must survive restarts or cross-host routing, do not rely on the default store. ([FastMCP][1])

### 9.11 Per-session visibility

`Context` can mutate the visible component graph for the current session only. The context docs say `ctx.enable_components()`, `ctx.disable_components()`, and `ctx.reset_visibility()` were added in `3.0.0`; the SDK says these session rules override global transforms, accumulate across calls, follow normal visibility-transform precedence, and send list-changed notifications only to the current session. Supported filter criteria include names/URIs, keys, version specs, tags, component kinds, and `match_all`. ([FastMCP][1])

```python id="jd7u98"
from fastmcp import FastMCP, Context
from fastmcp.dependencies import CurrentContext

mcp = FastMCP("Visibility")

@mcp.tool
async def enable_admin_mode(ctx: Context = CurrentContext()) -> str:
    await ctx.enable_components(tags={"admin"})
    return "admin tools visible in this session"

@mcp.tool
async def disable_experimental(ctx: Context = CurrentContext()) -> str:
    await ctx.disable_components(tags={"experimental"})
    return "experimental components hidden in this session"

@mcp.tool
async def reset_mode(ctx: Context = CurrentContext()) -> str:
    await ctx.reset_visibility()
    return "session visibility reset"
```

This is the correct primitive for session-personalized catalogs, namespace activation, guided workflows, or progressive disclosure of advanced capabilities. Do not use it as a substitute for authorization; it is visibility control, not a security boundary. The docs phrase it as session-only visibility rules layered over the global component graph. ([FastMCP][1])

### 9.12 Request metadata, session/request IDs, and request-context availability

`ctx.request_id` is the unique request identifier; `ctx.client_id` is the client identifier when available; `ctx.session_id` is the MCP session ID used for session-based data sharing and is available for all transports, using generated IDs where necessary; `ctx.session` exposes the underlying `ServerSession` for advanced usage. The context docs also emphasize that `ctx.request_context` may be `None` before the MCP session is established. ([FastMCP][1])

```python id="c92bxh"
@mcp.tool
async def ids(ctx: Context = CurrentContext()) -> dict:
    return {
        "request_id": ctx.request_id,
        "client_id": ctx.client_id,
        "session_id": ctx.session_id,
    }
```

For HTTP transports, MCP request context and raw HTTP request context are distinct. The context guide says `ctx.request_context` can be absent during `on_request` middleware or during connection initialization, while raw HTTP request data may still be available; in those cases use `get_http_request()` or `CurrentRequest()` instead. This is the correct boundary: `Context` is MCP-session-aware; raw HTTP request injection is transport-specific and available even when MCP session establishment has not completed. ([FastMCP][1])

### 9.13 Client-provided request metadata

Clients may attach contextual metadata through the MCP `meta` parameter, and the context docs say it becomes available as `ctx.request_context.meta` for tools, resources, and prompts. The field is `None` when not supplied; when present, it is accessed by attribute rather than dictionary lookup, and its shape is client-defined. ([FastMCP][1])

```python id="mwdglv"
@mcp.tool
def send_email(to: str, subject: str, body: str, ctx: Context) -> str:
    meta = ctx.request_context.meta if ctx.request_context else None
    user_id = meta.user_id if meta and hasattr(meta, "user_id") else None
    trace_id = meta.trace_id if meta and hasattr(meta, "trace_id") else None
    return f"user_id={user_id} trace_id={trace_id}"
```

Use request metadata for correlation IDs, tenant hints, UI context, or client-side provenance. Do not treat it as authenticated truth unless it is independently verified or derived from an authenticated access token. The docs describe the structure as client-determined metadata, not as an authorization artifact. ([FastMCP][1])

### 9.14 Transport awareness and extension awareness

`ctx.transport` returns `"stdio"`, `"sse"`, or `"streamable-http"` and is fixed for the running server instance. The context guide suggests transport-aware behavior such as shorter responses over STDIO or different timeout choices by transport. `ctx.client_supports_extension(extension_id)` checks client-advertised MCP extensions and returns `False` when there is no session or when the extension was not advertised. ([FastMCP][1])

```python id="xf2rba"
from fastmcp.apps.config import UI_EXTENSION_ID

@mcp.tool
async def maybe_rich(ctx: Context = CurrentContext()) -> str:
    if ctx.client_supports_extension(UI_EXTENSION_ID):
        return "UI-capable client"
    if ctx.transport == "stdio":
        return "text-only, local transport"
    return "text-only, remote transport"
```

Use these checks to adapt presentation or optional behaviors, not to encode core business semantics. Transport and extension support are execution-environment facts, not part of the user-domain model. ([FastMCP][1])

### 9.15 Background-task awareness

The SDK says `ctx.is_background_task()` is `True` when running inside a Docket worker; `ctx.task_id` gives the task ID; `ctx.origin_request_id` preserves the originating request ID when available. In that mode, `elicit()` and `sample()` switch to task-aware implementations that can pause the task and wait for client input, and `report_progress()` reports against task execution as well as foreground MCP requests. FastMCP updates also explicitly call out “Background Task Context” and distributed coordination improvements. ([FastMCP][2])

This matters architecturally because request-scoped context survives conceptually into worker execution even though the original HTTP request is gone. That makes `Context` the correct façade for task-aware interactive behavior, whereas raw `CurrentRequest()` should not be expected to exist outside HTTP-backed request contexts except for the minimal reconstructed form noted in the DI docs. ([FastMCP][2])

### 9.16 Adjacent DI primitives: when not to use `Context`

FastMCP’s DI system exposes more specific dependencies for cases where `Context` would be too broad:

* `CurrentRequest()` for the Starlette `Request` on HTTP transports; raises outside HTTP contexts.
* `CurrentAccessToken()` for the authenticated `AccessToken`; raises when unauthenticated.
* `CurrentFastMCP()` for the server instance itself.
* `get_http_request()`, `get_access_token()`, `get_server()`, and `get_context()` as function-level ambient-access helpers. ([FastMCP][3])

Use those narrower dependencies when the code only needs one precise runtime object. Example: if a tool only needs the user’s OAuth claims, inject `CurrentAccessToken()` rather than pulling `ctx.request_context` apart. If a tool only needs the raw HTTP user agent or client IP, use `CurrentRequest()` rather than mixing transport-specific request parsing into `Context` usage. If a helper only needs server introspection, `CurrentFastMCP()` is cleaner than `ctx.fastmcp`. ([FastMCP][3])

### 9.17 Design boundary: `Context` vs ordinary Python code

The clean architecture rule is: keep `Context` at the MCP edge. Use it in top-level tools/resources/prompts, or in thin adapter/helper layers reached from them. Convert MCP/runtime concerns into ordinary values as early as possible, then call pure business logic with plain Python types. This rule follows naturally from the docs’ framing of `Context` as the interface to MCP features and DI’s framing of runtime injection as a way to avoid passing context through every layer of an application. ([FastMCP][1])

Good boundary:

```python id="7vjgq0"
async def run_analysis(data: list[int], user_id: str | None) -> dict:
    return {"count": len(data), "user_id": user_id}

@mcp.tool
async def analyze(data: list[int], ctx: Context = CurrentContext()) -> dict:
    await ctx.info("Starting analysis")
    token = get_access_token()
    user_id = token.claims.get("sub") if token else None
    return await run_analysis(data, user_id)
```

Poor boundary: threading `Context` through domain services, repositories, or model classes that do not actually need MCP capabilities. That makes ordinary code untestable outside a request and couples business logic to transport/session state unnecessarily. The docs support the principle indirectly by presenting DI as a way to inject runtime values at the edge, not to redesign the whole application around `Context`. ([FastMCP][3])

### 9.18 Deployment advisories

For single-server STDIO/SSE/single-instance HTTP, session state set during initialization or earlier calls can persist straightforwardly when the same session object is reused. For distributed/serverless HTTP, state isolation and continuity rely on `mcp-session-id`, and the docs recommend custom storage backends like Redis/DynamoDB/Mongo-compatible `AsyncKeyValue` implementations instead of the default in-memory store. ([FastMCP][1])

For HTTP middleware and early connection phases, do not assume `ctx.request_context` is present. The docs explicitly say it can be `None` before MCP session establishment; use HTTP request helpers in those code paths. For background tasks, prefer `Context` methods like `report_progress()`, `sample()`, and `elicit()` over transport-specific request/session hacks, because the SDK gives those methods task-aware behavior. ([FastMCP][1])


[1]: https://gofastmcp.com/servers/context "MCP Context - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-server-context "context - FastMCP"
[3]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection - FastMCP"

# FastMCP Advanced — 10) Dependency injection
### Dependency injection

### 10.0 DI as a runtime contract, not a convenience layer

FastMCP treats dependency injection as a first-class runtime feature across tools, resources, templates, and prompts. The docs define the model simply: declare a parameter with either a recognized type annotation or a dependency default value, and FastMCP resolves it automatically at call time. Core DI features such as `Depends(...)`, `CurrentContext()`, and `CurrentFastMCP()` work without Docket; only background-task execution and task-specific dependencies like `CurrentDocket()` and `CurrentWorker()` require the `fastmcp[tasks]` extra. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

The design goal is separation of concerns: client/LLM-callable arguments stay in the MCP schema, while runtime-only values—context, request objects, access tokens, server handles, DB connections, task workers, progress reporters—are injected invisibly. The docs state this explicitly: dependency parameters are automatically excluded from the MCP schema, so clients never see them as callable parameters. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.1 Import surface

The recommended import surface is the centralized `fastmcp.dependencies` module, which re-exports the DI symbols for clean imports. The SDK page explicitly positions it as the single import location for dependency-related functionality. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-dependencies) ([FastMCP][2]))

```python id="qxgqqa"
from fastmcp.dependencies import (
    Depends,
    CurrentContext,
    CurrentRequest,
    CurrentAccessToken,
    CurrentFastMCP,
    CurrentDocket,
    CurrentWorker,
    Progress,
)
```

### 10.2 Public contract: what is hidden from MCP schemas

FastMCP hides dependency parameters from the exposed schema at two layers. Publicly, the docs say dependency parameters are excluded from MCP schemas so clients never see them. Internally, the server builds a schema-generation wrapper via `without_injected_parameters(...)` that removes `Context` and dependency parameters from the signature used for schema generation and validation, then re-injects them at execution time. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

This means the following signature exposes only `query` to clients, not `ctx` or `token`: ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="tawr11"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext, CurrentAccessToken
from fastmcp.server.context import Context
from fastmcp.server.auth import AccessToken

mcp = FastMCP("SchemaHygiene")

@mcp.tool
async def secure_search(
    query: str,
    ctx: Context = CurrentContext(),
    token: AccessToken = CurrentAccessToken(),
) -> dict:
    return {
        "query": query,
        "subject": token.claims.get("sub"),
        "request_id": ctx.request_id,
    }
```

### 10.3 Resolution order: effective runtime sequence

The public docs do not publish a named topological algorithm, but the documented behavior plus the current server source make the effective resolution sequence clear. First, `Context`-typed parameters are normalized into dependency defaults via `transform_context_annotations(...)`, unless they already have a dependency default such as `CurrentContext()`; for positional-or-keyword parameters, this may reorder parameters so defaulted injected params do not violate Python signature rules, while keyword-only parameter order is preserved. Second, schema generation uses a wrapper that strips injected parameters. Third, at call time `resolve_dependencies(...)` filters out any user-supplied keys that match dependency parameter names as a security measure. Fourth, `Depends(...)` dependencies are resolved recursively through the DI engine. Fifth, async-context-manager dependencies are entered and kept alive for the duration of the function call. Sixth, the final invocation kwargs are the filtered user args plus the resolved dependencies. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([FastMCP][3]))

A concise operational model is therefore:

1. normalize `Context` annotations into explicit DI defaults;
2. derive the public schema from only non-injected parameters;
3. drop any caller-supplied values for dependency parameter names;
4. resolve dependencies recursively;
5. cache shared subdependencies per request;
6. call the function with `filtered_user_args + resolved_dependencies`;
7. clean up context-manager dependencies after completion. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([GitHub][4]))

### 10.4 Security property: callers cannot override injected parameters

This is an explicit source-level guarantee. `resolve_dependencies(...)` filters out dependency-parameter names from user arguments before resolution, and the docstring states that this prevents external callers from overriding injected values. In other words, if a tool parameter is bound to `Depends(...)`, `CurrentContext()`, or another recognized dependency, a malicious or buggy client cannot smuggle a same-named argument through the wire payload and replace it. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/dependencies.py) ([GitHub][4]))

This is one of the strongest reasons to prefer DI over “hidden optional parameters with defaults” for sensitive/runtime-only values like access tokens, request objects, DB connections, server handles, or user IDs derived from auth. DI creates both schema hygiene and override protection. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/dependencies.py) ([GitHub][4]))

### 10.5 `CurrentContext()` and `Context`-typed injection

FastMCP supports two equivalent ways to request the active MCP `Context`: annotate a parameter as `Context`, or make the default `CurrentContext()`. The docs say both work identically; the explicit default is just more visibly “dependency-shaped” in the signature. Helper functions and middleware can use `get_context()` instead. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="5zfbdx"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

mcp = FastMCP("ContextDI")

@mcp.tool
async def process_data(data: str, ctx: Context = CurrentContext()) -> str:
    await ctx.info(f"Processing: {data}")
    return "Done"
```

Use `Context` when code must interact with MCP runtime facilities: logging, progress reporting, resource/prompt access, elicitation, sampling, session state, request/session identifiers, or visibility mutation. Do not use it as a generic “environment bag” for ordinary business inputs that should remain explicit and testable as plain parameters. ([gofastmcp.com](https://gofastmcp.com/servers/context) ([FastMCP][1]))

### 10.6 `CurrentFastMCP()` — server-handle injection

`CurrentFastMCP()` injects the active `FastMCP` server instance. The DI docs position it for introspection and server-level configuration access, while the SDK source adds an important background-task nuance: `get_server()` checks the task-server map first so mounted-child tasks resolve to the child server rather than the parent that owns the worker. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="nc17g1"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentFastMCP

mcp = FastMCP("ServerDI")

@mcp.tool
async def server_info(server: FastMCP = CurrentFastMCP()) -> str:
    return f"Server: {server.name}"
```

Use `CurrentFastMCP()` when code genuinely needs the server object—introspection, dynamic composition, or server-owned facilities. Do not inject the server only to reach unrelated business dependencies that should instead be expressed via `Depends(...)`. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.7 `CurrentRequest()` — HTTP-only request injection

`CurrentRequest()` injects the active Starlette `Request` object, but only for HTTP transports (SSE or Streamable HTTP). Both the prose docs and the SDK say it raises `RuntimeError` when no HTTP request is active, such as under STDIO. The docs also note that background tasks spawned from an HTTP request get a minimal synthetic request backed by snapshotted originating headers. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="jlwmel"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentRequest
from starlette.requests import Request

mcp = FastMCP("RequestDI")

@mcp.tool
async def client_info(request: Request = CurrentRequest()) -> dict:
    return {
        "user_agent": request.headers.get("user-agent", "Unknown"),
        "client_ip": request.client.host if request.client else "Unknown",
    }
```

Use `CurrentRequest()` only for transport-specific HTTP concerns: headers, client IP, cookies, or reverse-proxy details. If the code must also run over STDIO or mixed transports, prefer `CurrentHeaders()` or transport-agnostic abstractions instead of hard-depending on `Request`. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.8 `CurrentAccessToken()` — authenticated principal injection

`CurrentAccessToken()` injects the active authenticated `AccessToken` and raises if no authentication is present. The docs say the `AccessToken` exposes `client_id`, `scopes`, `expires_at`, and `claims`. The function-level alternative `get_access_token()` returns `None` instead of raising, and the SDK states it first prefers the current HTTP request scope, then falls back to the SDK auth context var, and in Docket workers falls back to the token snapshot stored at task-submission time. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="azf4qd"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentAccessToken
from fastmcp.server.auth import AccessToken

mcp = FastMCP("AuthDI")

@mcp.tool
async def current_user(token: AccessToken = CurrentAccessToken()) -> dict:
    return {
        "sub": token.claims.get("sub"),
        "client_id": token.client_id,
        "scopes": token.scopes,
    }
```

Use `CurrentAccessToken()` when authentication is required and absence should be a hard failure. Use `get_access_token()` when the behavior should degrade gracefully for anonymous or mixed-auth deployments. Never surface user IDs, scopes, or tenant IDs as plain MCP-callable arguments when they are already derivable from auth; inject them. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.9 `Depends(...)` — custom dependency injection

`Depends(...)` is the general-purpose DI primitive. The docs state that it wraps any callable and injects its return value, working with synchronous functions, asynchronous functions, and async context managers. This makes it the right mechanism for config injection, DB sessions, API clients, service objects, request-derived values, and other reusable runtime dependencies. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="jlwmp0"
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("DependsDemo")

def get_config() -> dict:
    return {"api_url": "https://api.example.com", "timeout": 30}

async def get_user_id() -> int:
    return 42

@mcp.tool
async def fetch_data(
    query: str,
    config: dict = Depends(get_config),
    user_id: int = Depends(get_user_id),
) -> str:
    return f"User {user_id} fetching '{query}' from {config['api_url']}"
```

Use `Depends(...)` whenever the value is runtime-only, security-sensitive, infrastructure-derived, or shared across multiple MCP components. Do not use it for caller-owned domain inputs that belong in the public schema. The docs explicitly frame custom dependencies as the path for config, database connections, API clients, and other non-client-supplied values. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.10 Nested dependencies

Nested dependencies are first-class. The DI docs state that dependencies can depend on other dependencies, and that FastMCP resolves them in the correct order. Public docs do not spell out a named graph algorithm, but the intended behavior is dependency-first resolution: leaf dependencies are resolved before the dependents that consume them, with caching applied across the tree. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="e3zgjk"
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("NestedDI")

def get_base_url() -> str:
    return "https://api.example.com"

def get_api_client(base_url: str = Depends(get_base_url)) -> dict:
    return {"base_url": base_url, "version": "v1"}

@mcp.tool
async def call_api(endpoint: str, client: dict = Depends(get_api_client)) -> str:
    return f"Calling {client['base_url']}/{client['version']}/{endpoint}"
```

This is the correct way to layer infrastructure factories: small leaf dependencies for atomic values, intermediate service constructors depending on those leaves, and top-level MCP components depending on the composed service objects. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.11 Caching behavior across the dependency tree

Dependencies are cached **per request**. The docs explicitly say that if multiple parameters use the same dependency, or if nested dependencies share a common dependency, FastMCP resolves it once and reuses the same instance. This is request-local caching, not a process-global singleton cache. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="r7ujct"
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("CachingDemo")

def get_db_connection():
    print("Connecting to database...")  # once per request
    return {"connection": "active"}

def get_user_repo(db=Depends(get_db_connection)):
    return {"db": db, "type": "user"}

def get_order_repo(db=Depends(get_db_connection)):
    return {"db": db, "type": "order"}

@mcp.tool
async def process_order(
    order_id: str,
    users=Depends(get_user_repo),
    orders=Depends(get_order_repo),
) -> str:
    return f"Processed order {order_id}"
```

The source backs this up: dependency resolution stores a per-invocation cache in `_Depends.cache`, resets it after the function completes, and merges the resolved dependencies into the final kwargs only after resolution finishes. That is why shared subdependencies are reused within one request but not leaked across requests. ([github.com](https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/dependencies.py) ([GitHub][4]))

### 10.12 Resource management via async context-manager dependencies

For dependencies that require cleanup—database sessions, HTTP clients, file handles, temporary resources—the docs recommend async context managers. Cleanup runs after the component function completes, even on error. This is the right pattern for finite-lifetime per-request resources; use server lifespan instead for long-lived shared infrastructure. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="ks8rcv"
from contextlib import asynccontextmanager
from fastmcp import FastMCP
from fastmcp.dependencies import Depends

mcp = FastMCP("ResourceManagedDI")

@asynccontextmanager
async def get_database():
    db = await connect_to_database()
    try:
        yield db
    finally:
        await db.close()

@mcp.tool
async def query_users(sql: str, db=Depends(get_database)) -> list:
    return await db.execute(sql)
```

This pattern composes naturally with caching. A shared context-manager dependency used multiple times in the same request still resolves once, and cleanup happens after the component call completes. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.13 Task-scoped dependencies: `Progress()`, `CurrentDocket()`, `CurrentWorker()`

Task dependencies are a separate tier. The DI docs and background-task guide both say they require `fastmcp[tasks]` and are only meaningful inside task-enabled components (`task=True`). FastMCP task mode is protocol-native background execution backed by Docket. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

```python id="qdr77j"
from docket import Docket, Worker
from fastmcp import FastMCP
from fastmcp.dependencies import Progress, CurrentDocket, CurrentWorker

mcp = FastMCP("TaskDI")

@mcp.tool(task=True)
async def long_running_task(
    data: str,
    progress: Progress = Progress(),
    docket: Docket = CurrentDocket(),
    worker: Worker = CurrentWorker(),
) -> str:
    await progress.set_total(100)
    for i in range(100):
        await progress.increment()
        await progress.set_message(f"Processing chunk {i + 1}")
    await docket.add(another_task, data)
    return f"Done via worker={worker.name}"
```

`CurrentDocket()` injects the active Docket instance so you can schedule more work, chain tasks, or access advanced Docket features. The SDK/source say it raises if Docket is not installed or if no task-enabled components caused Docket infrastructure to be initialized. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([FastMCP][3]))

`CurrentWorker()` injects the active Docket worker, giving access to worker metadata such as the worker name and concurrency settings. Like `CurrentDocket()`, it requires `fastmcp[tasks]` and task-enabled infrastructure. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

`Progress()` is the most deployment-sensitive task dependency. The SDK says it works in three execution modes: in a Docket worker it uses execution progress observable via Redis; in a FastMCP server with Docket but foreground execution it falls back to in-memory progress; and in a FastMCP server without Docket it also uses in-memory progress. The background-tasks guide adds that the same `Progress()` code works in both immediate and background execution modes. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([FastMCP][3]))

The `Progress` interface comprises `current()`, `total()`, `message()`, `set_total(total)`, `increment(amount=1)`, and `set_message(message)`. Those methods are the stable shared protocol between in-memory progress and Docket-backed worker progress. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([FastMCP][3]))

### 10.14 Best-practice hiding of sensitive/runtime-only values

Sensitive values should enter the call graph through DI, not the MCP schema. That includes access tokens, user/tenant IDs derived from claims, HTTP requests, server handles, DB sessions, API clients, Docket workers, and progress reporters. The docs explicitly state that dependency parameters are hidden from the MCP schema, and the source adds the security guarantee that user-supplied payloads cannot override dependency names. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

A strong default pattern is: public inputs stay explicit and typed; hidden infrastructure enters via DI; pure business logic receives ordinary values only. For example, prefer `user_id: str = TokenClaim("sub")` or `token: AccessToken = CurrentAccessToken()` over a public `user_id: str` argument when identity should come from authentication, not from the caller. The DI docs explicitly provide `TokenClaim(name)` as a narrow dependency for claim extraction. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.15 When to use DI vs ordinary parameters

Use ordinary parameters for caller-owned domain inputs: `query`, `amount`, `path`, `filters`, `entity_id`, `threshold`, `payload`. Use DI for execution-environment facts and runtime infrastructure: context, request, headers, token, server, configuration, service objects, DB connections, task infrastructure, and progress reporters. The docs’ examples consistently follow that split, and the schema-hiding behavior makes it the safest pattern for public MCP surfaces. ([gofastmcp.com](https://gofastmcp.com/servers/dependency-injection) ([FastMCP][1]))

### 10.16 Adjacent specialized built-ins

Even though the core feature set is `Depends(...)`, `CurrentContext()`, `CurrentRequest()`, `CurrentAccessToken()`, `CurrentFastMCP()`, `Progress()`, `CurrentDocket()`, and `CurrentWorker()`, the DI surface also exposes narrower helpers that are often a better fit than `Context`: `CurrentHeaders()` for transport-safe header access and `TokenClaim(name)` for single-claim extraction from the access token. The SDK documents both, and the DI guide shows when they are preferable to broader injections. ([gofastmcp.com](https://gofastmcp.com/python-sdk/fastmcp-server-dependencies) ([FastMCP][3]))


[1]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-dependencies "dependencies - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-server-dependencies "dependencies - FastMCP"
[4]: https://github.com/PrefectHQ/fastmcp/blob/main/src/fastmcp/server/dependencies.py "fastmcp/src/fastmcp/server/dependencies.py at main · PrefectHQ/fastmcp · GitHub"

# FastMCP Advanced — 11) Lifespans, session state, storage, and state ownership

## 11.0 State-domain mental model

FastMCP applications have several different kinds of state. Treating them as one interchangeable "context" is a common source of memory leaks, cross-user data exposure, and scaling failures.

```text
process / module state
    -> Python globals; usually avoid for mutable request/user state

FastMCP declaration state
    -> tools/resources/prompts/providers/transforms/middleware

server lifespan state
    -> initialized once per server start; shared across requests

request Context state
    -> valid for one MCP request

FastMCP 3 session state
    -> persists across requests in one MCP session

background-task state
    -> durable task metadata/result/progress in Docket backend
```

Use the narrowest state scope that satisfies the requirement. ([Lifespans][1]) ([Context][2])

---

## 11.1 Lifespan role

A FastMCP lifespan is the right owner for expensive shared resources that have explicit startup/shutdown behavior:

* DB connection pools;
* reusable HTTP clients;
* cloud SDK clients;
* loaded ML models;
* caches/indexes built once at boot;
* task worker infrastructure;
* telemetry exporters requiring cleanup.

Canonical pattern:

```python
from fastmcp import FastMCP
from fastmcp.server.lifespan import lifespan


@lifespan
async def app_lifespan(server):
    db = await open_database_pool()
    http = await open_http_client()
    try:
        yield {"db": db, "http": http}
    finally:
        await http.aclose()
        await db.close()


mcp = FastMCP("Service", lifespan=app_lifespan)
```

The yielded mapping is visible through request context. ([Lifespans][1])

---

## 11.2 Access lifespan state from a tool

```python
from fastmcp import Context


@mcp.tool
async def get_customer(customer_id: str, ctx: Context) -> dict:
    db = ctx.lifespan_context["db"]
    return await db.fetch_customer(customer_id)
```

Do not reconnect to the database on every call if a pool/client is naturally shareable.

---

## 11.3 Cleanup must be unconditional

Use `try/finally` around yielded state:

```python
@lifespan
async def app_lifespan(server):
    resource = await acquire()
    try:
        yield {"resource": resource}
    finally:
        await resource.close()
```

Avoid:

```python
@lifespan
async def app_lifespan(server):
    resource = await acquire()
    yield {"resource": resource}
    await resource.close()  # may be skipped on cancellation/failure paths
```

The lifecycle function is infrastructure code; cleanup semantics should be obvious under exceptions and cancellation.

---

## 11.4 Lifespan composition with ASGI hosts

`mcp.http_app()` creates an ASGI application with FastMCP lifecycle state. When mounting it into FastAPI/Starlette, the outer app must invoke that lifespan.

```python
mcp_app = mcp.http_app(path="/mcp")
app = FastAPI(lifespan=mcp_app.lifespan)
app.mount("/tools", mcp_app)
```

If the parent application already has its own lifespan, combine them rather than discarding either. See §19–20 for deployment patterns. FastMCP's HTTP docs explicitly warn that nested ASGI lifespans are not automatically run. ([HTTP Deployment][3])

---

## 11.5 Session state in FastMCP 3.x

FastMCP 3 provides session-scoped state through `Context`:

```python
@mcp.tool
async def increment(ctx: Context) -> int:
    count = await ctx.get_state("count") or 0
    count += 1
    await ctx.set_state("count", count)
    return count
```

Delete:

```python
await ctx.delete_state("count")
```

Session state is intended for values that should persist across requests within the same v3 MCP session, not server-global shared state. ([Context][2])

---

## 11.6 Serializable vs request-only values

Persistent session state must be representable by the configured store. Prefer JSON/Pydantic-serializable values:

```python
await ctx.set_state(
    "preferences",
    {"timezone": "UTC", "verbosity": "compact"},
)
```

For request-local nonserializable objects, FastMCP 3 can keep values only for the current request when explicitly marked nonserializable:

```python
await ctx.set_state("temporary_client", client, serializable=False)
```

Do not expect such a value to exist on a later MCP call. ([Context][2])

---

## 11.7 `session_state_store` constructor boundary

`FastMCP(...)` accepts a session state store abstraction. The v3 server constructor uses an in-memory store by default; an application can provide a persistent/distributed `AsyncKeyValue` implementation when session continuity must cross processes or restarts.

Conceptual construction:

```python
mcp = FastMCP(
    "StatefulService",
    session_state_store=my_async_key_value_store,
)
```

The exact backend constructor depends on the storage package/provider. Keep the FastMCP boundary abstract and configure the concrete backend in deployment code. ([Server][4])

---

## 11.8 In-memory store: appropriate uses

The default is appropriate for:

```text
local STDIO server
single process development HTTP server
unit/integration tests
state that can disappear on restart
one-server deployments where session affinity is guaranteed
```

It is not appropriate for:

```text
multiple Uvicorn workers
multiple replicas behind a load balancer
serverless instances
restart-sensitive workflows
state that must survive deploys
```

---

## 11.9 Distributed store requirements

A distributed session-state backend should have:

* atomic get/set/delete semantics appropriate to your state model;
* bounded TTL/lifecycle policy;
* tenant/session key isolation;
* serialization stability;
* observability for backend failures;
* predictable behavior during partial outages;
* versioned data structures if state schema can evolve.

Do not treat Redis/DynamoDB/Mongo/etc. as interchangeable merely because each can implement key/value operations; consistency and expiry semantics matter to application behavior.

---

## 11.10 State ownership table

| State | Owner | Lifetime | Example | Scaling requirement |
|---|---|---:|---|---|
| declaration | `FastMCP` / providers | process | component catalog | identical/reproducible across replicas |
| lifespan | server instance | process lifetime | DB pool | each replica initializes own resources |
| request | `Context` | one request | request ID, access token | no persistence expected |
| v3 session | session store | MCP session | conversational preference | shared store if requests cross workers |
| task | task backend | task lifetime | progress/result | durable/shared backend for workers |
| tenant config | app service/config | deployment/tenant | feature policy | explicit tenant isolation |

---

## 11.11 Never use module globals for user/session state

Bad:

```python
current_customer_id = None

@mcp.tool
async def select_customer(customer_id: str):
    global current_customer_id
    current_customer_id = customer_id
```

Concurrent sessions can overwrite one another. Instead use an explicit input, session state, authenticated identity, or application-level persistent store depending on semantics.

---

## 11.12 Lifespan state is shared, so make it concurrency-safe

A lifespan object can be used by many requests at the same time.

Safe examples:

* async DB pool designed for concurrency;
* `httpx.AsyncClient` reused concurrently;
* immutable configuration/model objects;
* concurrency-safe caches.

Potentially unsafe:

* a mutable cursor reused by all requests;
* a single non-thread-safe session object;
* mutable lists/dicts used as request scratch space;
* a client that requires thread affinity while sync tools run in worker threads.

Use per-request dependencies for non-shareable resources.

---

## 11.13 Dependency injection vs lifespan context

Two valid patterns:

```text
ctx.lifespan_context["db"]
    -> direct, simple, explicit FastMCP coupling

Depends(get_db)
    -> better abstraction, easier substitution/testing, nested dependencies
```

A dependency can itself retrieve lifespan state:

```python
from fastmcp.dependencies import CurrentContext, Depends


async def get_db(ctx: Context = CurrentContext()):
    return ctx.lifespan_context["db"]


@mcp.tool
async def get_order(order_id: str, db=Depends(get_db)) -> dict:
    return await db.get_order(order_id)
```

Use DI when multiple tools need the resource or when the service layer benefits from a typed dependency boundary.

---

## 11.14 State + authorization invariant

Do not use an unauthenticated client-supplied key as the sole namespace for sensitive state.

Bad conceptual design:

```text
state key = user_supplied_customer_id
```

Safer:

```text
state namespace = validated identity / tenant + session key + application subkey
```

FastMCP session isolation handles the MCP session boundary, but business-level tenant authorization remains your responsibility.

---

## 11.15 Background tasks are a separate persistence domain

Do not put long-running task status/results into session state if the FastMCP task system already owns them.

```text
session state
  -> conversational/request continuity

task backend
  -> task identity, progress, cancellation, result, worker coordination
```

Task execution may outlive an individual client request/session, especially when backed by external workers. See §12.

---

## 11.16 Stateless HTTP does not mean stateless application

FastMCP 3 HTTP can be configured `stateless_http=True` for load-balanced deployments. That setting changes transport/session behavior; it does not prohibit your application from using an external DB/cache/session service.

Design state explicitly:

```text
stateless transport + external application state = scalable normal pattern
```

Do not infer “no state anywhere” from `stateless_http`.

---

## 11.17 Session state and horizontal scaling in v3

If you require FastMCP 3 session state and deploy multiple workers/replicas:

1. use a shared `session_state_store`;
2. verify how the MCP session identifier is routed/preserved by your client and proxy;
3. avoid process-local mutable state as a hidden dependency;
4. test requests landing on different workers;
5. define expiry/cleanup behavior;
6. test deploy/restart behavior.

If you do not need session-scoped protocol features, stateless HTTP can simplify load balancing. See §20.

---

## 11.18 Request metadata is not lifespan state

Use `Context`/DI for:

* request ID;
* session ID;
* access token;
* HTTP request/header data;
* client capabilities;
* current visibility.

Do not copy these into globally shared lifespan state unless you are deliberately recording telemetry/audit data with appropriate synchronization and redaction.

---

## 11.19 State migration discipline

Persistent state is an API of its own. Version it.

```python
await ctx.set_state(
    "preferences",
    {
        "schema_version": 2,
        "timezone": "UTC",
        "display": {"compact": True},
    },
)
```

On read:

```python
prefs = await ctx.get_state("preferences")
if prefs and prefs.get("schema_version") == 1:
    prefs = migrate_v1_to_v2(prefs)
```

This is especially important when session stores survive rolling deploys where old and new replicas coexist.

---

## 11.20 Failure behavior

Decide deliberately what happens if the state backend is unavailable:

| State class | Typical failure policy |
|---|---|
| lifespan DB pool startup | fail server startup |
| optional cache | degrade / bypass cache |
| required session state | fail request rather than silently reset sensitive workflow |
| task backend | reject task creation / surface backend unavailable |
| telemetry exporter | often degrade, depending compliance requirements |

Avoid silently falling back from a required distributed store to local memory; that creates split-brain behavior across replicas.

---

## 11.21 Testing state

Test at least:

```text
same session: state persists
new session: state isolated
request-only nonserializable state: does not persist
server restart with memory store: state disappears
shared backend: state survives worker change
unauthorized tenant: cannot read another tenant's state
lifespan cleanup: close invoked on shutdown/error
concurrent requests: shared lifespan object remains correct
```

Use multiple `Client` instances when testing session isolation rather than reusing one client and assuming it creates a distinct session.

---

## 11.22 Anti-pattern inventory

* mutable module globals holding current user/customer/session;
* opening a DB pool per tool call;
* storing request scratch objects in the persistent session store;
* using session state as a substitute for durable business data;
* relying on memory state across replicas;
* putting secrets into state without evaluating storage encryption/access controls;
* using unauthenticated arbitrary IDs as state namespaces;
* assuming stateless HTTP makes external state impossible;
* silently falling back to per-process state when Redis/etc. is unavailable;
* forgetting to close lifespan resources;
* putting request-specific mutable objects into lifespan context;
* using task state and session state interchangeably.

---

## 11.23 Agent checklist

```text
[ ] Classify each value: declaration / lifespan / request / session / task / durable business state.
[ ] Use lifespan for expensive shared resources with cleanup.
[ ] Forward/combine lifespan when mounting ASGI apps.
[ ] Keep per-request mutable state out of lifespan globals.
[ ] Use Context/DI for request identity and protocol capabilities.
[ ] Use session state only for true cross-request session continuity.
[ ] Use a shared state store if sessions cross processes/replicas.
[ ] Keep task progress/results in the task backend.
[ ] Namespace sensitive state by validated identity/tenant.
[ ] Version persistent state schemas.
[ ] Test restart, worker-switch, and isolation behavior.
```

[1]: https://gofastmcp.com/servers/lifespan "Lifespans"
[2]: https://gofastmcp.com/servers/context "MCP Context"
[3]: https://gofastmcp.com/deployment/http "HTTP Deployment"
[4]: https://gofastmcp.com/servers/server "FastMCP Server"

# FastMCP Advanced — 12) Background tasks and long-running workflows
### Background tasks and long-running workflows

### 12.0 Capability model

FastMCP background tasks are **protocol-native**, not just an internal concurrency trick. The server-side task docs define the value proposition as: the client starts an operation, receives a task ID immediately, tracks progress while the work runs, and retrieves the result later through the MCP task protocol. The same docs explicitly distinguish this from plain Python concurrency primitives: you can still use asyncio/threads/processes or external queues, but MCP background tasks matter because MCP clients can manage them through the standard protocol surface rather than through ad hoc application APIs. ([FastMCP][1])

Client-side support is documented as the ability to request background execution with `task=True`, immediately receive a Task object, and then use a uniform API to poll, wait, cancel, or subscribe to status changes. This works for tools, resources, and prompts. ([FastMCP][2])

### 12.1 Enabling background execution on the server

The primary server-side switch is `task=True` on a component decorator. Current server docs say this works on **tools, resources, resource templates, and prompts**, and that when the client requests task execution the call returns immediately with a task ID while the work runs in a background worker. The docs are also explicit that task-enabled components must be **async functions**; using `task=True` on a synchronous function raises `ValueError` at registration time. Background-task support additionally requires the `fastmcp[tasks]` extra. ([FastMCP][1])

```python id="rly7iv"
import asyncio
from fastmcp import FastMCP

mcp = FastMCP("MyServer")

@mcp.tool(task=True)
async def slow_computation(duration: int) -> str:
    for i in range(duration):
        await asyncio.sleep(1)
    return f"Completed in {duration} seconds"
```

The same capability surface applies beyond tools. If an operation is naturally a long-running resource read, template resolution, or prompt rendering workflow, `task=True` can be attached there as well instead of forcing the abstraction into a tool purely to gain background execution. That is a significant design freedom: “long-running” and “tool” are not synonymous in FastMCP. ([FastMCP][1])

### 12.2 `TaskConfig` — explicit execution-mode control

Boolean `task=True` is just shorthand. The server task docs and the task-config SDK both define `TaskConfig` as the real execution-policy object for SEP-1686 task augmentation. The three modes are:

* `forbidden`: no task execution support
* `optional`: supports both synchronous and task execution
* `required`: client must request task execution or the call errors. ([FastMCP][1])

The public shorthand maps as follows: `task=True` becomes `TaskConfig(mode="optional")`; `task=False` becomes `TaskConfig(mode="forbidden")`. `TaskConfig.supports_tasks()` returns true for `optional` and `required`. `TaskConfig.validate_function(...)` enforces the two hard preconditions for task-enabled components: `fastmcp[tasks]` must be installed and the function must be async. ([FastMCP][1])

```python id="1a4zpk"
from fastmcp import FastMCP
from fastmcp.server.tasks import TaskConfig

mcp = FastMCP("TaskModes")

@mcp.tool(task=TaskConfig(mode="optional"))
async def flexible_task() -> str:
    return "Works either way"

@mcp.tool(task=TaskConfig(mode="required"))
async def must_be_background() -> str:
    return "Only runs as a background task"

@mcp.tool(task=TaskConfig(mode="forbidden"))
async def sync_only() -> str:
    return "Never runs as background task"
```

### 12.3 Poll interval and server-wide defaults

FastMCP lets the server advise clients how often to poll for task status. The server task docs say the default suggested poll interval is **5 seconds**, and that you can override it per component with `TaskConfig(mode=..., poll_interval=timedelta(...))`. This is an advisory value for polling clients, not a task timeout. ([FastMCP][1])

```python id="xx1mo3"
from datetime import timedelta
from fastmcp import FastMCP
from fastmcp.server.tasks import TaskConfig

mcp = FastMCP("Polling")

@mcp.tool(task=TaskConfig(mode="optional", poll_interval=timedelta(seconds=2)))
async def quick_task() -> str:
    return "Done quickly"

@mcp.tool(task=TaskConfig(mode="optional", poll_interval=timedelta(seconds=30)))
async def slow_task() -> str:
    return "Eventually done"
```

At the server level, `FastMCP(..., tasks=True)` enables task support by default for all components. The task docs also warn that if the server contains any synchronous tools, resources, or prompts, those components must explicitly set `task=False` to avoid errors, because task-enabled components must be async. ([FastMCP][1])

```python id="73vq2k"
from fastmcp import FastMCP

mcp = FastMCP("GlobalTasks", tasks=True)

@mcp.tool(task=False)
def fast_sync_lookup(key: str) -> str:
    return f"value:{key}"
```

### 12.4 Immediate-return task IDs and low-level task metadata

At the high level, client code requests background execution with `task=True`:

```python id="zr0nri"
from fastmcp import Client

async with Client(server) as client:
    task = await client.call_tool("slow_computation", {"duration": 10}, task=True)
    print(task.task_id)
```

The client task docs say this same pattern works with `call_tool(...)`, `read_resource(...)`, and `get_prompt(...)`, returning `ToolTask`, `ResourceTask`, and `PromptTask` respectively. ([FastMCP][2])

Under the hood, the server-side task routing/config layer models task augmentation as `TaskMeta`. The task-config SDK defines `TaskMeta(ttl=None, fn_key=None)` where `ttl` is a client-requested TTL in milliseconds and `fn_key` is the Docket routing key, auto-derived from the component name if omitted. The routing SDK further says task routing receives `task_type`, `arguments`, and `task_meta`, then either submits to Docket or executes synchronously depending on mode and request shape. For agents building lower-level adapters or custom routing logic, `TaskMeta` is the real protocol-side control object behind the simple `task=True` API. ([FastMCP][3])

### 12.5 Graceful degradation: client contract and a documentation caveat

At the **client** layer, the task docs are unambiguous: you can always pass `task=True`; if the server does not support background execution, the operation executes immediately and the Task object reports `returned_immediately=True`. The Task SDK reinforces this: `returned_immediately()` is true when the server executed synchronously because of graceful degradation or no task support. ([FastMCP][2])

At the **server** layer, the high-level task guide says that when a client requests background execution but the component is `mode="forbidden"`, FastMCP executes synchronously and returns the inline result, while `mode="required"` without task augmentation yields an error. ([FastMCP][1])

However, the lower-level SDK reference is less forgiving: `TaskConfig` and task-routing documentation say `mode="forbidden"` with task augmentation returns `-32601` / raises `McpError`, and `mode="required"` without task augmentation also errors. Because the official docs conflict here, the safest authoring policy is: do not rely on `mode="forbidden"` plus `task=True` as a compatibility path. If you want graceful dual behavior, use `mode="optional"`. Reserve `mode="forbidden"` and `mode="required"` for hard contract enforcement. ([FastMCP][3])

### 12.6 Task backends: in-memory vs Redis/Valkey

FastMCP’s server-side task execution is backed by Docket. The task docs define two supported backends configured through `FASTMCP_DOCKET_URL`: `memory://` by default, or `redis://host:port/db` for production. ([FastMCP][1])

The in-memory backend requires no external dependencies and is fine for simple single-process deployment, but the docs explicitly list three drawbacks: it is ephemeral across server restarts, has higher pickup latency (about 250 ms), and does not support horizontal scaling. Redis/Valkey is the recommended production backend because tasks survive restarts, pickup latency drops to single-digit milliseconds, and additional workers can distribute load across processes or machines. ([FastMCP][1])

That yields a crisp deployment rule: use `memory://` only for local development, tests, or single-process internal services where durability and scale do not matter. Use Redis/Valkey whenever task continuity, worker scaling, or production latency matters. ([FastMCP][1])

### 12.7 Worker topology and scaling

Every FastMCP server with task-enabled components automatically starts an **embedded worker**, so a separate worker process is not required just to make background tasks run. The docs then describe horizontal scaling as adding more workers via the CLI, all pulling from the same queue. Worker concurrency is configured with `FASTMCP_DOCKET_CONCURRENCY`. Additional workers only work with Redis/Valkey; the in-memory backend is single-process only. ([FastMCP][1])

```bash id="0wll1g"
export FASTMCP_DOCKET_URL=redis://localhost:6379/0
export FASTMCP_DOCKET_CONCURRENCY=20
fastmcp tasks worker server.py
```

A crucial operational constraint is also explicit: task-enabled components must exist at **server startup** so all workers can register them. Components added dynamically after startup are not available for background execution. For agent-authored systems with plugin loading or late registration, this means all task-capable components must be loaded before workers start. ([FastMCP][1])

### 12.8 Progress reporting

Server-side progress reporting uses the `Progress` dependency. The task docs define the injection pattern and the API surface as `set_total(n)`, `increment(amount=1)`, and `set_message(text)`, and say the same code works in both immediate and background execution modes. The DI SDK adds that `Progress()` dispatches to different implementations depending on execution mode: Docket-backed execution progress in workers, in-memory execution progress in foreground/task-capable servers, and in-memory progress in servers without Docket. ([FastMCP][1])

```python id="x4h8bw"
from fastmcp import FastMCP
from fastmcp.dependencies import Progress

mcp = FastMCP("ProgressDemo")

@mcp.tool(task=True)
async def process_files(files: list[str], progress: Progress = Progress()) -> str:
    await progress.set_total(len(files))
    for file in files:
        await progress.set_message(f"Processing {file}")
        # ... work ...
        await progress.increment()
    return f"Processed {len(files)} files"
```

Use `Progress` for any task or long-running operation where the client benefits from intermediate visibility. Because the same dependency works in immediate and background execution, it is the right primitive even when a component supports both modes via `TaskConfig(mode="optional")`. ([FastMCP][1])

### 12.9 Docket-aware task dependencies

FastMCP exposes Docket-specific dependencies inside task-enabled functions. The server task docs show `CurrentDocket()` and `CurrentWorker()` alongside `Progress()`, and explain that these enable advanced workflows such as scheduling additional background tasks, chaining work, inspecting worker metadata, and leveraging Docket features like retries and timeouts. ([FastMCP][1])

```python id="o17l3u"
from docket import Docket, Worker
from fastmcp import FastMCP
from fastmcp.dependencies import Progress, CurrentDocket, CurrentWorker

mcp = FastMCP("TaskInfra")

@mcp.tool(task=True)
async def my_task(
    progress: Progress = Progress(),
    docket: Docket = CurrentDocket(),
    worker: Worker = CurrentWorker(),
) -> str:
    await progress.set_total(10)
    await docket.add(another_task, "payload")
    worker_name = worker.name
    return f"scheduled via {worker_name}"
```

These dependencies should be treated as task-runtime infrastructure, not public API inputs. Use them inside task-enabled components or deeper helper layers called from them; do not expose equivalent routing/worker concepts through user-callable MCP arguments. ([FastMCP][1])

### 12.10 Client-side Task objects

FastMCP’s client task API is unified around an abstract `Task` base class with subclasses `ToolTask`, `PromptTask`, and `ResourceTask`. The SDK says all three provide a common API whether the server truly accepted background execution or executed immediately through graceful degradation. The specific `result()` return type then matches the underlying operation: `CallToolResult` for tools, `GetPromptResult` for prompts, and resource-content lists for resources. ([FastMCP][4])

`task.task_id` returns the server task ID. `task.returned_immediately()` reports whether execution happened synchronously instead of in the background. `task.status()` gets current task status; if the task ran immediately, it returns a synthetic completed status instead of making a live status request. `task.wait(state=None, timeout=None)` blocks until a desired state or terminal completion, using event-based waiting when notifications are available and polling as a fallback. `task.cancel()` sends a `tasks/cancel` request, and becomes a no-op when the server executed immediately. `task.on_status_change(callback)` registers sync or async callbacks for `notifications/tasks/status` updates when the server supports them. ([FastMCP][4])

Representative client flow:

```python id="lmjtjl"
from fastmcp import Client

async with Client(server) as client:
    task = await client.call_tool("slow_computation", {"duration": 10}, task=True)

    if task.returned_immediately():
        result = await task.result()
    else:
        task.on_status_change(lambda s: print(f"{s.taskId}: {s.status} - {s.statusMessage}"))
        status = await task.status()
        final_status = await task.wait(timeout=30.0)
        if final_status.status == "completed":
            result = await task.result()
        elif final_status.status == "failed":
            ...
        elif final_status.status == "cancelled":
            ...
```

The high-level client docs also spell out the common status values as `working`, `completed`, `failed`, and `cancelled`, and show `task.wait(state="completed", timeout=30.0)` as the controlled waiting API. ([FastMCP][2])

### 12.11 `result()`, `status()`, `wait()`, `cancel()`, `on_status_change(...)`

`result()` is the “give me the final domain result” API. For `ToolTask`, the SDK says it returns the same parsed tool result as `call_tool`; for `PromptTask`, the prompt result with messages and description; for `ResourceTask`, the final resource contents. If the server executed immediately, `result()` returns the immediate result without an extra server round trip. ([FastMCP][4])

`status()` is the non-blocking status check. The high-level client guide shows `status.status` and `status.statusMessage`; the SDK says immediate execution returns a synthetic completed status instead of querying the server. Use this when you need current state but do not want to await completion. ([FastMCP][2])

`wait()` is the controlled blocking primitive. The SDK says it can wait for a named intermediate/final state (`submitted`, `working`, `completed`, `failed`) or any terminal state when `state=None`, and that it prefers event-driven wakeups from task-status notifications with fallback polling for reliability. This is the best primitive when the client wants bounded waiting with deterministic timeout behavior. ([FastMCP][4])

`cancel()` issues a protocol cancellation request. The SDK says the server will attempt to halt execution and move the task to `cancelled`; if execution already happened synchronously, `cancel()` is a no-op because there is no server-side task to stop. Use `cancel()` for user-abort semantics, not for structured retry logic. ([FastMCP][4])

`on_status_change(...)` is the event-subscription primitive. The SDK says callbacks may be sync or async, and they fire when `notifications/tasks/status` messages arrive; the high-level client docs show a handler receiving `taskId`, `status`, and `statusMessage`. Because those notifications are an optional server feature, `on_status_change(...)` is best treated as a latency optimization layered on top of `wait()`/`status()`, not as the only correctness path. ([FastMCP][2])

### 12.12 Server-side and client-side “graceful degradation” together

The client docs explicitly endorse always passing `task=True` because servers without task support may execute inline and the Task object will reflect that through `returned_immediately()`. That makes the client API uniform even when capabilities differ across deployments. ([FastMCP][2])

Server authors should complement that with deliberate `TaskConfig` choices. `optional` is the compatibility mode for dual sync/background behavior. `required` is the contract-hardening mode when blocking execution is unacceptable or semantically wrong. `forbidden` is the task-free mode for components that must never leave the foreground path, with the earlier caveat that high-level and low-level docs disagree on whether a task-augmented request degrades inline or errors. ([FastMCP][1])

### 12.13 Deployment advisories

Use background tasks for **operations you know are long-running**, rather than simply stretching foreground timeouts. The tools docs are explicit that timeouts protect foreground execution only; `timeout` does not apply to background tasks, and long-running work should use `task=True` so clients can poll for progress while execution happens in workers. ([FastMCP][5])

Prefer `TaskConfig(mode="optional")` when you need one component to support both synchronous and background invocation. Prefer `mode="required"` when immediate execution would be a bad UX or resource-management choice. Avoid leaning on `mode="forbidden"` with `task=True` requests because the current official docs conflict on whether that degrades or errors. ([FastMCP][1])

Use the in-memory backend only for development and single-process deployments. For production workloads requiring persistence, low latency, or horizontal worker scale, set `FASTMCP_DOCKET_URL` to Redis/Valkey and add workers via `fastmcp tasks worker ...`. Configure `FASTMCP_DOCKET_CONCURRENCY` explicitly when worker throughput matters. ([FastMCP][1])

Define all task-enabled components at startup. The docs are explicit that dynamically added components are not registered with already-running workers and therefore are unavailable for background execution. In plugin-heavy systems, this means “discover plugins before starting workers,” not after. ([FastMCP][1])

Inject `Progress()`, `CurrentDocket()`, and `CurrentWorker()` rather than passing task/runtime objects as public parameters. These are infrastructure concerns, not MCP-callable business inputs, and FastMCP’s DI system is designed precisely to keep them out of the public schema. ([FastMCP][1])


[1]: https://gofastmcp.com/servers/tasks "Background Tasks - FastMCP"
[2]: https://gofastmcp.com/clients/tasks "Background Tasks - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-server-tasks-config "config - FastMCP"
[4]: https://gofastmcp.com/python-sdk/fastmcp-client-tasks "tasks - FastMCP"
[5]: https://gofastmcp.com/servers/tools "Tools - FastMCP"

# FastMCP Advanced — 13) Middleware and the server policy layer
### Middleware and the server policy layer

### 13.0 Scope and version frame

FastMCP middleware is a **FastMCP-specific** server feature, not part of the core MCP protocol. The middleware system was introduced in `2.9.0` and is explicitly framed by the docs as the place for cross-cutting behavior such as authentication, logging, rate limiting, request/response transformation, caching, and error handling without modifying individual tools, resources, or prompts. ([FastMCP][1])

The correct mental model is: middleware wraps the **server’s public MCP operations**. Requests flow inward through the middleware chain; results flow outward in reverse order. In FastMCP’s own server SDK, `call_tool`, `read_resource`, and `render_prompt` apply middleware by default, and they expose `run_middleware=False` specifically so middleware can call back into server operations without recursively reapplying the chain. ([FastMCP][2])

### 13.1 Where middleware lives in the object model

Middleware is attached to the `FastMCP` server and therefore belongs to the **server policy layer**, not to individual component definitions. The server SDK exposes `add_middleware(self, middleware: Middleware) -> None`, and the middleware guide shows `mcp.add_middleware(...)` as the standard attachment API. Because server listing/execution methods already layer visibility filtering, auth filtering, and middleware execution on top of provider aggregation, middleware should be understood as the outermost request/response policy ring around the resolved component graph. ([FastMCP][3])

Minimal attachment pattern:

```python id="dpg7ym"
from fastmcp import FastMCP
from fastmcp.server.middleware.logging import LoggingMiddleware

mcp = FastMCP("MyServer")
mcp.add_middleware(LoggingMiddleware())
```

This attaches one middleware instance to the server’s global request pipeline; it is not scoped to just one tool or one provider. ([FastMCP][4])

### 13.2 Pipeline semantics and `call_next(context)`

FastMCP middleware is a bidirectional pipeline. The middleware guide states the flow explicitly as `Request → Middleware A → Middleware B → Handler → Middleware B → Middleware A → Response`. The controlling primitive is `call_next(context)`: calling it continues the chain; not calling it terminates processing at the current middleware. That makes middleware the correct place for pre-processing, post-processing, short-circuiting, and policy enforcement. ([FastMCP][2])

```python id="dx2jws"
from fastmcp import FastMCP
from fastmcp.server.middleware import Middleware, MiddlewareContext

class SimpleLogger(Middleware):
    async def on_message(self, context: MiddlewareContext, call_next):
        print(f"→ {context.method}")
        result = await call_next(context)
        print(f"← {context.method}")
        return result

mcp = FastMCP("MyServer")
mcp.add_middleware(SimpleLogger())
```

This is the canonical hook shape: inspect before, delegate, inspect/transform after, return the downstream result. ([FastMCP][4])

### 13.3 Execution order and stack design

Middleware executes in the order it is added. The first middleware runs first on the inbound path and last on the outbound path. The docs explicitly recommend placing error handling early so it can catch exceptions from downstream middleware and handlers, and placing logging later so it observes the post-policy execution path rather than the raw input alone. ([FastMCP][2])

```python id="n9x6lf"
from fastmcp import FastMCP
from fastmcp.server.middleware.error_handling import ErrorHandlingMiddleware
from fastmcp.server.middleware.rate_limiting import RateLimitingMiddleware
from fastmcp.server.middleware.logging import LoggingMiddleware

mcp = FastMCP("OrderedServer")
mcp.add_middleware(ErrorHandlingMiddleware())  # 1st in, last out
mcp.add_middleware(RateLimitingMiddleware())   # 2nd in, 2nd out
mcp.add_middleware(LoggingMiddleware())        # 3rd in, first out
```

In practice, a robust stack often looks like: error handling near the front, then auth/rate limiting, then caching where appropriate, then logging/timing near the edge. The exact stack is policy-dependent, but the ordering rule itself is fixed by FastMCP. ([FastMCP][2])

### 13.4 Mounted servers and middleware hierarchy

When servers are mounted, middleware follows a parent/child hierarchy. The middleware guide states that parent middleware runs for **all** requests, including requests routed to mounted servers, while child middleware runs only for operations handled by that mounted child. A request to a mounted child tool therefore flows through the parent’s middleware first and then the child’s middleware. ([FastMCP][2])

This matters operationally: global auth, tenancy, tracing, or rate limiting belongs naturally on the parent; child-specific logging or domain policy belongs on the mounted child. If you attach auth only to the child, parent-only requests bypass it; if you attach logging only to the parent, child-local detail may be lost unless the parent logs enough metadata. ([FastMCP][2])

### 13.5 Hook hierarchy: message, request/notification, operation

FastMCP provides hook specificity levels rather than forcing everything through one universal message interceptor. The docs define three levels:

* `on_message` for all MCP traffic, both requests and notifications
* `on_request` / `on_notification` for traffic split by response expectation
* operation hooks such as `on_call_tool`, `on_read_resource`, `on_get_prompt`, `on_list_tools`, `on_list_resources`, `on_list_resource_templates`, `on_list_prompts`, and `on_initialize` for specific operations. ([FastMCP][2])

The hook order for a tool call is explicitly documented as `on_message` → `on_request` → `on_call_tool`. This gives a clean design rule: use `on_message` for full-funnel logging/metrics, `on_request` for request-wide checks like auth or rate limiting, and `on_call_tool` only when you truly need tool-specific logic. ([FastMCP][2])

### 13.6 Hook signature and `MiddlewareContext`

Every hook follows the same signature pattern:

```python id="bedjlwm"
async def hook_name(self, context: MiddlewareContext, call_next) -> result_type:
    result = await call_next(context)
    return result
```

The middleware guide and SDK agree that `context` is the unified middleware context object and `call_next` is the async continuation function. `MiddlewareContext` exposes at least these fields: `method`, `source`, `type`, `message`, `timestamp`, and `fastmcp_context`. The SDK also exposes `copy(...)` on `MiddlewareContext` for producing modified copies when needed. ([FastMCP][2])

The most important fields in practice are:

* `context.method`: MCP method name like `"tools/call"`
* `context.message`: the operation payload object
* `context.fastmcp_context`: request-scoped FastMCP `Context`, when available. ([FastMCP][2])

### 13.7 Message hooks

`on_message` runs for every MCP message, including both requests and notifications. The middleware guide recommends it for logging, metrics, and any concern that should observe the entire traffic surface. The logging middleware built-ins also anchor themselves at this level. ([FastMCP][2])

Use `on_message` when the distinction between tool/resource/prompt/listing is irrelevant and you care about the full stream of protocol events. Avoid putting operation-specific policy here unless you genuinely need a one-hook-for-everything design. ([FastMCP][2])

### 13.8 Request hooks and notification hooks

`on_request` runs only for MCP requests that expect a response. The docs recommend it for authentication, authorization, and request validation. `on_notification` runs only for fire-and-forget notifications and is appropriate for event logging or asynchronous side effects. Notifications do not return values. ([FastMCP][2])

This split matters because many cross-cutting policies should apply only to request-response operations. For example, rate limiting belongs naturally on `on_request`, not on `on_message`, if you do not want server-originated notifications or other non-request traffic to count against quotas. FastMCP’s own `RateLimitingMiddleware` follows that pattern by implementing `on_request`. ([FastMCP][5])

### 13.9 Operation hooks

Operation hooks let middleware target specific MCP operations. The middleware guide documents at least:

* `on_call_tool`
* `on_read_resource`
* `on_get_prompt`
* `on_list_tools`
* `on_list_resources`
* `on_list_resource_templates`
* `on_list_prompts`
* `on_initialize`. ([FastMCP][6])

For example, `on_call_tool` receives a context whose message contains `name` and `arguments`; `on_read_resource` sees `uri`; listing hooks return FastMCP model objects like `list[Tool]`, `list[Resource]`, `list[ResourceTemplate]`, or `list[Prompt]`. This is the right hook tier for operation-specific policies such as tool-name-based auth, prompt caching, or resource URI rewriting. ([FastMCP][2])

### 13.10 `on_initialize`: handshake policy and a hard timing rule

`on_initialize` runs when a client connects and initializes the session. The middleware guide is explicit that this hook **cannot modify** the initialization response. If you want to reject a client, you must raise `McpError` **before** `call_next(context)`; raising after `call_next()` only logs the error because the response has already been sent. ([FastMCP][2])

That makes `on_initialize` the correct place for early client admission control, compatibility gating, or client-info inspection, but not for post hoc response mutation. For policy that truly needs to alter later request behavior rather than the handshake itself, use later hooks plus session state. ([FastMCP][2])

### 13.11 Raw handler: overriding `__call__`

If hook dispatch is too granular or too opinionated, you can bypass it by overriding `__call__` directly on a `Middleware` subclass. The docs state that this gives complete control over all messages and bypasses the hook dispatch system entirely. Use it when you want one uniform handler regardless of message type or operation. ([FastMCP][2])

```python id="56y4sk"
from fastmcp.server.middleware import Middleware, MiddlewareContext

class RawMiddleware(Middleware):
    async def __call__(self, context: MiddlewareContext, call_next):
        print(f"Processing: {context.method}")
        result = await call_next(context)
        print(f"Completed: {context.method}")
        return result
```

This is powerful but blunt. Prefer specialized hooks when possible; use raw `__call__` when the policy truly does not care about request type or operation specificity. ([FastMCP][2])

### 13.12 Session availability inside middleware

`context.fastmcp_context` may exist before a full MCP session is established, but `ctx.request_context` may still be absent during phases like initialization. The middleware guide explicitly warns that session-specific attributes may be unavailable early, and recommends checking before using them. For HTTP-specific fallback data like headers or client IP during those phases, use the HTTP dependency helpers such as `get_http_headers()`. ([FastMCP][2])

This is one of the most important “gotchas” for middleware authors. Do not assume session ID, request ID, or other session-backed fields are always available in early hooks. Middleware that must run both before and after initialization should branch on session availability instead of unconditionally dereferencing session-bound data. ([FastMCP][2])

### 13.13 Built-in logging middleware

FastMCP ships `LoggingMiddleware` and `StructuredLoggingMiddleware`. The middleware guide says the first produces human-readable request/response logs, while the second emits JSON-structured logs suitable for aggregation systems like Splunk or Datadog. The logging SDK describes them as comprehensive request/response logging middleware built on a common base class. ([FastMCP][2])

```python id="yaojbh"
from fastmcp import FastMCP
from fastmcp.server.middleware.logging import LoggingMiddleware

mcp = FastMCP("LoggingDemo")
mcp.add_middleware(
    LoggingMiddleware(
        include_payloads=True,
        max_payload_length=1000,
    )
)
```

The documented configuration surface includes `include_payloads`, `max_payload_length`, and an optional custom `logger`. Use human-readable logging in development and structured JSON logging in environments where centralized log ingestion or analytics matter. ([FastMCP][2])

### 13.14 Built-in timing middleware

FastMCP ships `TimingMiddleware` and `DetailedTimingMiddleware`. The middleware guide says `TimingMiddleware` logs execution duration for all requests, while the SDK says it operates on request messages rather than notifications. `DetailedTimingMiddleware` breaks timing down by operation type, including tools, resource reads, prompt retrieval, and list operations. ([FastMCP][2])

```python id="0p8dwx"
from fastmcp import FastMCP
from fastmcp.server.middleware.timing import TimingMiddleware, DetailedTimingMiddleware

mcp = FastMCP("TimingDemo")
mcp.add_middleware(TimingMiddleware())
# or:
# mcp.add_middleware(DetailedTimingMiddleware())
```

Use `TimingMiddleware` for coarse service latency visibility. Use `DetailedTimingMiddleware` when you need per-operation performance attribution, especially on servers mixing expensive tool calls with relatively cheap list/read operations. ([FastMCP][7])

### 13.15 Response caching middleware

`ResponseCachingMiddleware` is the built-in caching layer. The middleware guide says it caches tool calls, resource reads, and list operations with TTL-based expiration; the SDK adds that it also caches `prompts/get` and all supported list methods, supports invalidation via server notifications, and can use storage backends with features like LRU eviction or size limits. ([FastMCP][2])

```python id="51u4mo"
from fastmcp import FastMCP
from fastmcp.server.middleware.caching import (
    ResponseCachingMiddleware,
    CallToolSettings,
    ListToolsSettings,
    ReadResourceSettings,
)

mcp = FastMCP("CachingDemo")
mcp.add_middleware(
    ResponseCachingMiddleware(
        list_tools_settings=ListToolsSettings(ttl=30),
        call_tool_settings=CallToolSettings(included_tools=["expensive_tool"]),
        read_resource_settings=ReadResourceSettings(enabled=False),
    )
)
```

The guide lists per-operation settings classes such as `ListToolsSettings`, `ListResourcesSettings`, `ListPromptsSettings`, `CallToolSettings`, `ReadResourceSettings`, and `GetPromptSettings`. Each accepts at least `enabled`, `ttl`, and include/exclude allowlist/denylist controls for specific items. ([FastMCP][2])

For persistence or distributed deployments, the guide shows plugging in a different storage backend such as a file-tree-backed `AsyncKeyValue` store. The storage-backends docs separately state that FastMCP storage defaults to in-memory and that persistent/distributed backends are the right choice when restart durability or multi-process sharing is required. ([FastMCP][2])

The most important caveat is explicit in both the middleware guide and the SDK: **cache keys are derived from operation name and arguments only**. They do **not** include user identity or session identity. If a tool returns user-specific data derived from auth context, headers, or session state rather than explicit request arguments, you must either disable caching for that operation or make the relevant identity dimension part of the arguments. ([FastMCP][2])

A second, subtler caveat also comes from the SDK: cached results are no longer the original object instances but no-op/cache wrapper objects, so response caching may be incompatible with other middleware that expects original subclasses or exact runtime object identity. That means caching should usually sit **after** middleware that depends on original result subclasses, or those middleware should operate on normalized data rather than concrete subclass behavior. ([FastMCP][8])

### 13.16 Built-in rate limiting middleware

FastMCP provides `RateLimitingMiddleware` and `SlidingWindowRateLimitingMiddleware`. The SDK says the default rate limiter uses a **token bucket** algorithm, allowing bursts while enforcing a sustainable long-term rate, and the middleware guide documents `max_requests_per_second`, `burst_capacity`, and optional `client_id_func` as key parameters. The sliding-window variant is more precise but uses more memory to track timestamps. ([FastMCP][5])

```python id="cins8f"
from fastmcp import FastMCP
from fastmcp.server.middleware.rate_limiting import (
    RateLimitingMiddleware,
    SlidingWindowRateLimitingMiddleware,
)

mcp = FastMCP("RateLimited")
mcp.add_middleware(
    RateLimitingMiddleware(
        max_requests_per_second=10.0,
        burst_capacity=20,
    )
)

# Alternative:
# mcp.add_middleware(
#     SlidingWindowRateLimitingMiddleware(max_requests=100, window_minutes=1)
# )
```

Use token-bucket rate limiting for most production servers because it handles bursty traffic well. Use sliding-window rate limiting when you need tighter fairness/precision and can afford the extra memory cost. That preference is an engineering inference from the documented algorithm tradeoffs. ([FastMCP][5])

### 13.17 Error-handling middleware

FastMCP ships `ErrorHandlingMiddleware` and `RetryMiddleware` in `fastmcp.server.middleware.error_handling`. The middleware guide says `ErrorHandlingMiddleware` centralizes error logging and transformation, while the SDK says it catches exceptions, logs them appropriately, converts them to proper MCP error responses, and tracks error patterns for monitoring through `get_error_stats()`. ([FastMCP][2])

```python id="85n7bz"
from fastmcp import FastMCP
from fastmcp.server.middleware.error_handling import ErrorHandlingMiddleware

mcp = FastMCP("Errors")
mcp.add_middleware(
    ErrorHandlingMiddleware(
        include_traceback=True,
        transform_errors=True,
        error_callback=my_error_callback,
    )
)
```

The documented configuration surface includes `include_traceback`, `transform_errors`, and an optional `error_callback`. Place this middleware early in the stack so it can catch downstream middleware and handler exceptions. That ordering recommendation is explicit in the middleware guide. ([FastMCP][2])

### 13.18 Retry middleware

`RetryMiddleware` is the built-in retry layer. The SDK says it implements automatic retry logic for transient failures with exponential backoff; the middleware guide gives `max_retries` and `retry_exceptions` as key configuration inputs. ([FastMCP][9])

```python id="1d2lyt"
from fastmcp import FastMCP
from fastmcp.server.middleware.error_handling import RetryMiddleware

mcp = FastMCP("Retries")
mcp.add_middleware(
    RetryMiddleware(
        max_retries=3,
        retry_exceptions=(ConnectionError, TimeoutError),
    )
)
```

Because retries replay requests, this middleware is best reserved for operations where replay is acceptable or desirable; that is an engineering recommendation inferred from the middleware’s documented behavior, not a separate FastMCP rule. In practice, it is a better fit for transient network failures or flaky upstreams than for destructive non-idempotent actions. ([FastMCP][9])

### 13.19 Ping middleware

`PingMiddleware` sends periodic server-to-client pings to keep long-lived connections alive. The middleware guide says it is most useful for **stateful HTTP connections** and has no effect on stateless connections; the SDK adds that it starts a background ping task on the first message from each session and keeps pinging until the session ends. The documented default interval is `30000` ms, configurable with `interval_ms`. ([FastMCP][2])

```python id="8rcr2w"
from fastmcp import FastMCP
from fastmcp.server.middleware import PingMiddleware

mcp = FastMCP("KeepAlive")
mcp.add_middleware(PingMiddleware(interval_ms=5000))
```

Use `PingMiddleware` when the deployment relies on long-lived stateful connections that may be dropped by proxies, clients, or idle timeouts. Do not expect it to help stateless request models. ([FastMCP][2])

### 13.20 Custom middleware subclass patterns

FastMCP’s custom middleware story is subclass-based: derive from `Middleware`, override only the hooks you care about, and unoverridden hooks pass through automatically. The middleware guide states this directly and shows constructor-based configuration for middleware instances. ([FastMCP][2])

#### Denying requests

The guide is explicit: raise the appropriate exception type to stop processing and return an error to the client. The correct exception type depends on the operation: `ToolError` for tool calls, `ResourceError` for resource reads, `PromptError` for prompt retrieval, and `McpError` for general requests. Do **not** signal errors by returning special values or by silently skipping `call_next()`; raise. ([FastMCP][2])

```python id="rd3qyo"
from fastmcp.server.middleware import Middleware, MiddlewareContext
from fastmcp.exceptions import ToolError

class AuthMiddleware(Middleware):
    async def on_call_tool(self, context: MiddlewareContext, call_next):
        if context.message.name in {"delete_all", "admin_config"}:
            raise ToolError("Access denied: requires admin privileges")
        return await call_next(context)
```

#### Modifying requests

Request mutation is performed by altering `context.message` before delegation. The guide shows normalizing a tool argument in `context.message.arguments` before calling the next middleware/handler. ([FastMCP][2])

```python id="rhooa9"
from fastmcp.server.middleware import Middleware, MiddlewareContext

class InputSanitizer(Middleware):
    async def on_call_tool(self, context: MiddlewareContext, call_next):
        if context.message.name == "search":
            query = context.message.arguments.get("query", "")
            context.message.arguments["query"] = query.strip().lower()
        return await call_next(context)
```

#### Modifying responses

Post-processing is done after `await call_next(context)`. The guide shows augmenting a tool’s `structured_content` after execution. It also notes that if the transformation is fundamentally a component-rewrite concern rather than a request/response concern, a Transform may be the better abstraction. ([FastMCP][2])

```python id="n7n9gz"
from fastmcp.server.middleware import Middleware, MiddlewareContext

class ResponseEnricher(Middleware):
    async def on_call_tool(self, context: MiddlewareContext, call_next):
        result = await call_next(context)
        if context.message.name == "get_data" and result.structured_content:
            result.structured_content["processed_by"] = "enricher"
        return result
```

### 13.21 Filtering list results and maintaining execution consistency

List-operation hooks return FastMCP model objects and can be filtered before they reach the client. The middleware guide explicitly warns that if you filter list results—for example, removing private tools from `on_list_tools`—you should also block execution in the corresponding operation hook to keep the published list consistent with actual server behavior. ([FastMCP][2])

```python id="0h13ck"
from fastmcp.server.middleware import Middleware, MiddlewareContext
from fastmcp.exceptions import ToolError

class PrivateToolFilter(Middleware):
    async def on_list_tools(self, context: MiddlewareContext, call_next):
        tools = await call_next(context)
        return [tool for tool in tools if "private" not in tool.tags]

    async def on_call_tool(self, context: MiddlewareContext, call_next):
        if context.fastmcp_context:
            tool = await context.fastmcp_context.fastmcp.get_tool(context.message.name)
            if "private" in tool.tags:
                raise ToolError("Tool not found")
        return await call_next(context)
```

This pattern matters for trustworthiness. If the client cannot list a tool but can still call it successfully, your published contract is incoherent. The docs explicitly recommend filtering and blocking together. ([FastMCP][2])

### 13.22 Accessing component metadata inside middleware

Execution-hook contexts do not directly expose component metadata like tags. The guide says the correct pattern is to look up the component through the server using `context.fastmcp_context.fastmcp.get_tool(...)`, `get_resource(...)`, or `get_prompt(...)`. This is how middleware can implement tag-based authorization, classification, or routing decisions. ([FastMCP][2])

```python id="ve8ovf"
from fastmcp.server.middleware import Middleware, MiddlewareContext

class TagBasedAuth(Middleware):
    async def on_call_tool(self, context: MiddlewareContext, call_next):
        if context.fastmcp_context:
            tool = await context.fastmcp_context.fastmcp.get_tool(context.message.name)
            if tool and "requires-auth" in tool.tags:
                # enforce auth policy here
                ...
        return await call_next(context)
```

This lookup path also respects the server’s own visibility/version rules, which is generally what you want when writing request-time policy code. ([FastMCP][2])

### 13.23 Storing state in middleware for later use

The middleware guide shows middleware writing state into the FastMCP context so later tools can read it. The example uses HTTP headers, derives a `user_id`, and writes it through `context.fastmcp_context.set_state(...)`, with later tool code reading it via `ctx.get_state(...)`. This is the policy-layer way to enrich request handling without bloating public schemas. ([FastMCP][2])

```python id="uj50ma"
from fastmcp.server.middleware import Middleware, MiddlewareContext
from fastmcp.server.dependencies import get_http_headers

class UserMiddleware(Middleware):
    async def on_request(self, context: MiddlewareContext, call_next):
        headers = get_http_headers() or {}
        user_id = headers.get("x-user-id", "anonymous")
        if context.fastmcp_context:
            context.fastmcp_context.set_state("user_id", user_id)
        return await call_next(context)
```

This is appropriate for request-derived enrichment or correlation data. It is not a substitute for real authorization or for durable session-state design when data must persist beyond one request/session boundary. ([FastMCP][2])

### 13.24 Error handling inside custom middleware

The middleware guide explicitly recommends wrapping `call_next()` in `try/except` when a custom middleware needs to observe downstream failures, and then **re-raising** the exception. Catching without re-raising suppresses the error entirely. That is the correct pattern for instrumentation or auditing middleware that needs to log failures but still preserve protocol-correct error behavior. ([FastMCP][2])

```python id="77r5di"
from fastmcp.server.middleware import Middleware, MiddlewareContext

class ErrorLogger(Middleware):
    async def on_request(self, context: MiddlewareContext, call_next):
        try:
            return await call_next(context)
        except Exception as e:
            print(f"Error in {context.method}: {type(e).__name__}: {e}")
            raise
```

### 13.25 Practical policy-layer advisory

Use middleware for **cross-cutting** policy: auth, quotas, timing, caching, logging, and request/response normalization across multiple components. Use component code for component-local business logic. Use transforms when you want to change the published component graph itself rather than the live request/response flow. The middleware guide itself points to transforms for more complex tool transformations, which is the cleanest statement of the boundary. ([FastMCP][2])

The highest-risk operational mistake is unsafe caching. Because cache keys ignore user/session identity and depend only on operation name and arguments, caching any operation whose effective result depends on auth context, headers, or session state can leak data across users. That caveat is explicit in the docs and should be treated as a hard design rule, not a footnote. ([FastMCP][2])


[1]: https://gofastmcp.com/changelog "Changelog"
[2]: https://gofastmcp.com/servers/middleware "Middleware - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-server-server "server - FastMCP"
[4]: https://gofastmcp.com/servers/middleware "Middleware"
[5]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-rate_limiting "rate_limiting - FastMCP"
[6]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-middleware "middleware - FastMCP"
[7]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-timing "timing - FastMCP"
[8]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-caching "caching - FastMCP"
[9]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-error_handling "error_handling - FastMCP"

# FastMCP Advanced — 14) Providers and dynamic component sources
### Providers and dynamic component sources

### 14.0 Mental model: source layer vs rewrite layer vs policy layer

FastMCP v3 split server composition into two distinct layers. **Providers** are the **component source layer**: they answer “what tools/resources/prompts exist, and where do they come from?” **Transforms** are the **component-rewrite layer**: they answer “how should those components be renamed, filtered, reshaped, or bridged before clients see them?” Middleware remains the later **request/response policy layer**. If an agent confuses providers with transforms, it will tend to solve source problems with request hooks or solve publication problems with component copies. The official docs explicitly frame providers as sources and transforms as in-flight modifiers on the path from providers to clients. ([FastMCP][1])

The shortest correct pipeline is:

`Provider(s) → provider-level transforms → server-level transforms → server filtering/visibility/auth → client-visible catalog`

and, for execution, the resolved component then runs through the server’s normal middleware/auth/visibility path. Provider transforms run first; server transforms run after them and see already-transformed names. ([FastMCP][2])

### 14.1 Providers: the component source layer

Every FastMCP server has one or more providers. When a client lists tools, FastMCP asks each provider for tools and combines the results; when a client calls a specific component, FastMCP finds the provider that owns it and delegates accordingly. The docs are explicit that `@mcp.tool` adds to the server’s `LocalProvider`, so even “simple one-file servers” are already using the provider architecture whether the author notices or not. ([FastMCP][1])

Provider order is semantically significant. The provider overview states that `LocalProvider` is always first, so decorator-defined components take precedence over added providers, and additional providers are queried in the order they were added. If two providers expose the same component name, the first one wins. That makes provider order a real API-design concern rather than an implementation detail. ([FastMCP][3])

The base `Provider` contract is intentionally narrow: override protected `_list_*` methods for the component types you source; optionally override `_get_*` methods only if you can resolve a specific component more efficiently than scanning list results. The public `list_*`/`get_*` methods are already transform-aware. The custom-provider docs also state that providers return **ready-to-run component objects**; when a client calls a tool, FastMCP invokes the returned `Tool` object’s execution path, not some opaque provider callback. In other words, providers source executable components rather than “descriptions of components.” ([FastMCP][4])

Representative custom-provider skeleton:

```python id="lvn0sx"
from collections.abc import Sequence
from fastmcp.server.providers import Provider
from fastmcp.tools import Tool
from fastmcp.resources import Resource
from fastmcp.prompts import Prompt

class MyProvider(Provider):
    async def _list_tools(self) -> Sequence[Tool]:
        return []

    async def _list_resources(self) -> Sequence[Resource]:
        return []

    async def _list_prompts(self) -> Sequence[Prompt]:
        return []
```

This is the canonical provider extension point documented by FastMCP. ([FastMCP][4])

### 14.2 `LocalProvider`: code-defined components

`LocalProvider` is the default provider storing components you define directly in code through `@mcp.tool`, `@mcp.resource`, `@mcp.prompt`, or programmatic `add_*` methods. In practice, if you never compose or proxy anything, `LocalProvider` is the only provider you touch, but it is still the same provider abstraction underneath. This is why local components take precedence over mounted or proxied ones by default. ([FastMCP][1])

The engineering implication is simple: local definitions are the safest place for overrides. If you proxy or mount an upstream server and then need to replace one tool with a local variant, the local copy wins because the local provider is queried first. That precedence rule is core to safe composition. ([FastMCP][3])

### 14.3 `FastMCPProvider`: wrapping another FastMCP server

`FastMCPProvider` wraps another `FastMCP` server and exposes its components through the provider interface. The SDK page highlights the crucial behavior: wrapped components delegate execution to the child server’s middleware path, so child-server middleware still runs when the parent invokes those components. This is the correct source abstraction for **in-process server composition**. ([FastMCP][5])

FastMCP automatically uses this wrapper when you add or mount a `FastMCP` server as a provider. The aggregate-provider SDK explicitly says that if the provider is a FastMCP server, it is automatically wrapped in `FastMCPProvider` to ensure middleware is invoked correctly. That means `mount(child)` is not “copy these components into the parent”; it is “wrap the child as a live provider and preserve its execution semantics.” ([FastMCP][6])

This wrapper-based delegation is what makes mounted composition safe: child middleware, child auth, child versioning, and child execution behavior continue to exist instead of being flattened away at registration time. If you want a live child-server boundary, use `mount()`, not ad hoc import/copy patterns. ([FastMCP][5])

### 14.4 `ProxyProvider`: remote/component proxying

`ProxyProvider` is the remote-source analogue. The proxy-provider docs define it as sourcing components from another MCP server through a client connection, exposing that remote server’s tools, resources, templates, and prompts through your own server whether the backend is local or networked. The stated use cases are transport bridging, backend aggregation, controlled gateways, and stable front-door endpoints over changing backends. ([FastMCP][7])

The recommended top-level construction path is `create_proxy(...)`. The server SDK documents `create_proxy(target, **settings) -> FastMCPProxy` and says it accepts a connected or disconnected `Client`, a transport, a `FastMCP` instance, a URL, a script `Path`, or an MCP config dict. For most users, `create_proxy(...)` is the correct primitive; drop to `ProxyProvider` or `FastMCPProxy` only when you need lower-level control. ([FastMCP][8])

Minimal proxy server:

```python id="102gy0"
from fastmcp.server import create_proxy

proxy = create_proxy("http://example.com/mcp", name="MyProxy")

if __name__ == "__main__":
    proxy.run()
```

This is the canonical quick-start shape documented by FastMCP. ([FastMCP][7])

`ProxyProvider` returns proxy component objects—`ProxyTool`, `ProxyResource`, `ProxyTemplate`, and `ProxyPrompt`—that forward execution to the backend server. The SDK is explicit that all proxied components have `task_config.mode="forbidden"` because background tasks cannot execute through a proxy. It is also explicit that component lists are cached, with default `cache_ttl=300` seconds, shared across sessions, refreshed on `list_*` calls, and disableable via `cache_ttl=0`; disabling is recommended for dynamic backends whose catalogs change often. ([FastMCP][9])

This yields two important design rules. First, do not expect background-task semantics to survive across a generic proxy boundary; a proxy can mirror ordinary operations and advanced protocol forwarding, but proxied components are not task-capable. Second, disable proxy catalog caching when the upstream server’s component list changes dynamically, otherwise name/metadata staleness will be your default behavior. ([FastMCP][9])

### 14.5 Proxy session models: isolated vs shared

`create_proxy()` defaults to **session isolation**: each request gets its own isolated backend session. The proxy docs call this the recommended mode and explicitly say it prevents context mixing when multiple clients use the proxy concurrently. ([FastMCP][7])

If you instead pass an already-connected client to `create_proxy(...)`, the proxy reuses that session. The docs warn that all requests then share one backend session, which may cause context mixing under concurrency; this should be reserved for single-threaded or explicitly synchronized situations. ([FastMCP][7])

This is a major deployment advisory: default to isolated sessions unless you have a backend that is intentionally stateful and you fully control concurrency. Shared-session proxying is an optimization/special case, not the safe default. ([FastMCP][7])

### 14.6 Proxy feature forwarding

FastMCP’s proxy layer is not limited to tools/resources/prompts. The proxy-provider SDK documents default forwarding handlers for roots, sampling, elicitation, logging, and progress, and the `ProxyClient` class is described as forwarding these advanced interactions between the remote server and the proxy’s connected clients. This is why FastMCP proxying is more than just “HTTP fetch and replay”; it forwards the richer MCP feature surface as well. ([FastMCP][9])

For agent authors, the consequence is positive but bounded: proxying preserves a surprising amount of MCP richness, but it still does not make proxied components locally task-capable and it still inherits the operational caveats of remote latency, caching, and session strategy. ([FastMCP][9])

### 14.7 `FileSystemProvider`: filesystem discovery / hot-reload source

`FileSystemProvider` is the convention-over-coordination provider. The docs say it scans a directory for Python files, imports them, and registers any standalone-decorated `Tool`, `Resource`, `ResourceTemplate`, or `Prompt` objects it finds. The key architectural point is that files use standalone decorators (`@tool`, `@resource`, `@prompt`) and do not need access to a server instance. ([FastMCP][10])

```python id="jlwmga"
from pathlib import Path
from fastmcp import FastMCP
from fastmcp.server.providers import FileSystemProvider

mcp = FastMCP(
    "MyServer",
    providers=[FileSystemProvider(Path(__file__).parent / "mcp")]
)
```

This is the documented quick-start pattern. ([FastMCP][11])

The SDK surface for `FileSystemProvider` is small but important: `root` controls the directory to scan, and `reload=True` rescans on every request. The docs present reload mode as a development-time convenience and the default `reload=False` as scan-once-at-init caching behavior. Use reload only when you are actively editing discovered files; leave it off in production to avoid repeated import/discovery overhead. ([FastMCP][10])

One additional nuance: the filesystem provider supports the normal standalone decorator option surfaces. The docs explicitly say the standalone `@tool` decorator used in discovered files still supports the standard tool options such as `name`, `title`, `description`, `icons`, `tags`, `output_schema`, `annotations`, and `meta`. That means filesystem discovery is not a “reduced capability mode”; it is merely a different source layer. ([FastMCP][12])

### 14.8 Skills providers: exposing skills as MCP resources

Skills providers are specialized providers that expose agent-skill directories as MCP resources. The skills docs define a skill as a directory centered on `SKILL.md` plus optional supporting files, and say the provider exposes those skills as discoverable/readable `skill://...` resources. This is a resource-first abstraction, not a tool abstraction. ([FastMCP][13])

The core provider forms are:

* `SkillProvider` for one skill directory
* `SkillsDirectoryProvider` for one or more roots containing many skills
* vendor-specific providers such as `ClaudeSkillsProvider`, `CursorSkillsProvider`, `VSCodeSkillsProvider`, `CodexSkillsProvider`, `GeminiSkillsProvider`, `GooseSkillsProvider`, `CopilotSkillsProvider`, and `OpenCodeSkillsProvider`, which preconfigure platform-default roots. ([FastMCP][13])

```python id="okuq1q"
from pathlib import Path
from fastmcp import FastMCP
from fastmcp.server.providers.skills import SkillsDirectoryProvider

mcp = FastMCP("Skills Server")
mcp.add_provider(
    SkillsDirectoryProvider(roots=Path.home() / ".claude" / "skills")
)
```

This is the documented general pattern. ([FastMCP][13])

The resource model is explicit: each skill exposes a main file resource like `skill://pdf-processing/SKILL.md`, a synthetic manifest `skill://pdf-processing/_manifest`, and optionally supporting-file resources. `SkillsDirectoryProvider` can scan multiple roots, and the docs say the first root takes precedence if the same skill name appears in multiple locations. ([FastMCP][13])

A particularly important configuration knob is `supporting_files=`. With `supporting_files="template"`—the default—supporting files are hidden from `list_resources()` and exposed through a `ResourceTemplate`, keeping the catalog compact while allowing manifest-driven discovery. With `supporting_files="resources"`, every supporting file appears as an individual resource in `list_resources()`. Use template mode when you want smaller catalogs; use resources mode when clients need flat, fully enumerable file visibility. The same page also documents `reload=True` for re-scanning on every request, again positioning it as a development convenience rather than a production default. ([FastMCP][13])

### 14.9 Custom providers: when to build one

The custom-provider docs say you build a custom provider when components come from somewhere other than code-defined decorators, child FastMCP servers, or remote proxies: databases, external APIs, configuration systems, multi-tenant catalogs, or plugin systems. The provider abstraction is the correct place to source those components. ([FastMCP][4])

The same docs also draw an important architectural boundary: it is often cleaner to let a provider source **all possible components**, then use middleware or visibility controls to filter what each request can actually see. In other words, source selection and request-specific publication are different concerns. Do not try to encode every per-request policy into the provider itself if visibility/middleware can do it later more cleanly. ([FastMCP][4])

Custom providers also get lifecycle management via `lifespan()`. The docs show providers using `lifespan()` to open DB/API connections at server startup and close them at shutdown, with FastMCP calling the provider lifespan automatically. This is the correct place for provider-owned connection pools or clients. ([FastMCP][4])

### 14.10 Transforms: the component-rewrite layer

Transforms modify components **as they flow** from providers to clients. The transforms overview describes them as pipeline filters; when listing, transforms are pure sequence-to-sequence functions, and when resolving a specific component by name/URI they behave like middleware, mapping the client-facing name back to the original before returning the transformed result. This dual list/get behavior is the key conceptual model. ([FastMCP][2])

Built-in transforms currently include `Namespace`, `ToolTransform`, `Enabled`, `Tool Search`, `ResourcesAsTools`, `PromptsAsTools`, and experimental `Code Mode`. Of those, the highest-value composition transforms for most servers are `Namespace`, `ToolTransform`, `ToolSearch`, `ResourcesAsTools`, and `PromptsAsTools`. ([FastMCP][2])

### 14.11 Provider-level vs server-level transforms

Transforms can be attached either to a provider or to the server. Provider-level transforms affect one source and run first; server-level transforms affect the aggregated component graph and run after provider transforms. The transforms overview makes this ordering explicit and shows that server-level transforms see already-transformed names. ([FastMCP][2])

This yields a strong authoring rule:

* use **provider-level transforms** for source-local rewrites, such as namespacing or schema cleanup of one mounted/proxied source;
* use **server-level transforms** for global publication policy, such as version-prefixing or whole-server search replacement. ([FastMCP][2])

Transforms stack in the order added; the first added is innermost, and later transforms wrap it. The overview explicitly demonstrates `verbose_name -> api_verbose_name -> short` as the flow when `Namespace` is added first and a later `ToolTransform` renames the namespaced tool. This ordering matters when composing namespace + rewrite + visibility transforms. ([FastMCP][2])

### 14.12 `add_transform(...)` vs `wrap_transform(...)`

On providers, `add_transform(...)` mutates the provider in place, while `wrap_transform(...)` returns a new provider wrapper with the transform applied and leaves the original unchanged. The provider-base SDK explicitly recommends `wrap_transform(...)` when the same provider will be reused in multiple aggregators with different transform stacks, such as different namespaces. ([FastMCP][14])

Use `add_transform(...)` when the provider instance is single-owner and mutable by design. Use `wrap_transform(...)` when the provider is a reusable building block and you want composition without side effects. This is one of the most important ergonomics rules in the whole provider system. ([FastMCP][14])

### 14.13 `Namespace` — conflict prevention by systematic prefixing

`Namespace("api")` prefixes all component names from a source. The namespace docs say tools and prompts get underscore-separated prefixes, while resources and templates get URI path-segment prefixes. The canonical examples are:

* tool `my_tool` → `api_my_tool`
* prompt `my_prompt` → `api_my_prompt`
* resource `data://info` → `data://api/info`
* template `data://{id}` → `data://api/{id}`. ([FastMCP][15])

This is the default safety transform for composition and the most common transform used by `mount(..., namespace="...")`. In a multi-server parent, namespacing should be treated as the default unless you are intentionally constructing one flat shared namespace and fully control collisions. ([FastMCP][15])

### 14.14 `ToolTransform` — schema and interface rewriting

`ToolTransform` is the deferred tool-schema rewrite mechanism. The transform page says it is used when tools flow through a transform chain—especially tools from mounted servers, proxies, or other providers you do not control directly—and that it can rename tools, change descriptions, adjust tags, and reshape argument schemas. ([FastMCP][16])

Representative transform:

```python id="uxlpz6"
from fastmcp import FastMCP
from fastmcp.server.transforms import ToolTransform
from fastmcp.tools.tool_transform import ToolTransformConfig

mcp = FastMCP("TransformDemo")

@mcp.tool
def verbose_internal_data_fetcher(query: str) -> str:
    return f"Results for: {query}"

mcp.add_transform(ToolTransform({
    "verbose_internal_data_fetcher": ToolTransformConfig(
        name="search",
        description="Search the database.",
    )
}))
```

Clients now see `search` instead of the original tool name. That exact pattern is documented. ([FastMCP][16])

The lower-level tool-transform SDK exposes the argument-rewrite vocabulary through `ArgTransform`. Supported per-argument rewrites include renaming, description changes, default/default_factory insertion, type changes, hiding an argument, forcing requiredness, and adding examples. `TransformedTool.from_tool(...)` additionally supports overriding tool name, version, title, description, tags, annotations, output schema, and meta, with helper functions `forward()` and `forward_raw()` for custom forwarding logic. This is the deeper API behind `ToolTransform` and the right place to look when schema surgery must become exact. ([FastMCP][17])

One useful distinction from the transform docs: `ToolTransform` is the **deferred** rewrite mechanism; `Tool.from_tool()` / `TransformedTool.from_tool()` are the **immediate** object-level rewrite mechanism. Use deferred transforms when you do not control the source or want composition-time rewrites; use immediate object transforms when you already have the tool object and want to register a rewritten copy. ([FastMCP][16])

### 14.15 `ToolSearch` — discovery compression for large catalogs

`ToolSearch` is the transform family for replacing full tool listings with on-demand discovery. The docs state that when a search transform is installed, `list_tools()` returns only two synthetic tools:

* `search_tools`
* `call_tool`

while the original tools remain fully callable but hidden from listing. Search covers tool names, descriptions, parameter names, and parameter descriptions, and search results come back in the same JSON format as `list_tools`, including full input schema. ([FastMCP][18])

FastMCP currently documents two search strategies:

* `RegexSearchTransform`, which uses case-insensitive regex matching, has zero indexing overhead, and is the best fit when the model can formulate targeted patterns;
* `BM25SearchTransform`, which uses BM25 ranking, builds an in-memory index lazily, rebuilds it when tool text changes, and is better for natural-language discovery queries. ([FastMCP][18])

By default, search returns up to 5 tools. Both transforms support `max_results`; they also support `always_visible` to pin specific tools into the listing alongside the synthetic search tools, and configurable synthetic tool names if `search_tools` / `call_tool` would collide with real tool names. The docs also state that the `call_tool` proxy resolves the target through the server’s **normal** tool pipeline, including transforms and middleware, and explicitly rejects recursive calls to the synthetic tools themselves. ([FastMCP][18])

Use ToolSearch when catalog size is large enough that dumping all tools into LLM context harms cost or selection quality. This is one of the clearest examples of transforms as “client-catalog UX rewrites” rather than source rewrites. ([FastMCP][18])

### 14.16 `ResourcesAsTools` — bridge resources to tool-only clients

Some MCP clients cannot speak the resource protocol at all. `ResourcesAsTools` solves that by generating two synthetic tools, `list_resources` and `read_resource`, which route through the server at runtime so normal middleware, auth, visibility, and rate limiting still apply. The docs explicitly say the transform should be applied to a **FastMCP server instance**, not to a raw Provider, because the generated tools need a server to route back through. If you want only a subset of resources exposed this way, create a dedicated subserver for those resources and apply the transform there. ([FastMCP][19])

```python id="4k9cap"
from fastmcp import FastMCP
from fastmcp.server.transforms import ResourcesAsTools

mcp = FastMCP("ResourceBridge")

@mcp.resource("config://app")
def app_config() -> str:
    return '{"app_name": "My App", "version": "1.0.0"}'

mcp.add_transform(ResourcesAsTools(mcp))
```

The generated `list_resources` output distinguishes static resources (`uri`) from templates (`uri_template`), and `read_resource` accepts a concrete URI, including filled-in template URIs. The docs also note that binary content read through `read_resource` is base64-encoded in the tool response. Both generated tools are marked `readOnlyHint=True`. ([FastMCP][19])

### 14.17 `PromptsAsTools` — bridge prompts to tool-only clients

`PromptsAsTools` is the prompt analogue. It generates `list_prompts` and `get_prompt` tools, again routing through the server’s normal middleware chain, and again should be applied to a FastMCP server rather than a raw Provider. If only a subset of prompts should be exposed this way, the docs explicitly recommend building a dedicated subserver for that subset and applying the transform there. ([FastMCP][20])

```python id="c8i5v9"
from fastmcp import FastMCP
from fastmcp.server.transforms import PromptsAsTools

mcp = FastMCP("PromptBridge")

@mcp.prompt
def analyze_code(code: str, language: str = "python") -> str:
    return f"Analyze this {language} code:\n{code}"

mcp.add_transform(PromptsAsTools(mcp))
```

`list_prompts` returns prompt metadata plus arguments, while `get_prompt` takes a prompt name and optional arguments dict and returns standard MCP-format messages. This transform is valuable whenever the client can only call tools but your server’s “real” high-level reusable assets are prompts. ([FastMCP][20])

### 14.18 `mount(...)` — safe in-process composition

`mount(...)` is the primary server-composition primitive for bringing another `FastMCP` server under a parent. The composition docs frame it as a **live link**, not a snapshot: if you add components to the child after mounting, they become immediately available through the parent. This is the right primitive for modular monorepos, feature servers, and in-process orchestration layers. ([FastMCP][21])

```python id="x6c8fx"
from fastmcp import FastMCP

weather = FastMCP("Weather")
calendar = FastMCP("Calendar")

@weather.tool
def get_data() -> str:
    return "Weather data"

@calendar.tool
def get_data() -> str:
    return "Calendar data"

main = FastMCP("Main")
main.mount(weather, namespace="weather")
main.mount(calendar, namespace="calendar")
```

This is the documented namespacing pattern for conflict-safe composition. ([FastMCP][21])

One underused but important detail from the transforms overview: the provider returned by `mount()` can itself receive provider-level transforms. That means you can namespace at mount time and then add a further `ToolTransform` to only that mounted source without touching the child server or the parent’s whole catalog. ([FastMCP][2])

### 14.19 `create_proxy(...)` and mounting external servers

The composition docs show `mount(create_proxy(...), namespace="...")` as the canonical pattern for combining remote or subprocess-backed servers under one parent. `create_proxy()` accepts URLs directly, local Python script paths, MCP config dicts (including npm/uvx command specs), and explicit transport objects. This makes it the “bring something external under the provider graph” primitive. ([FastMCP][21])

Representative external composition:

```python id="7via7x"
from fastmcp import FastMCP
from fastmcp.server import create_proxy

mcp = FastMCP("Orchestrator")

mcp.mount(create_proxy("http://api.example.com/mcp"), namespace="api")
mcp.mount(create_proxy("./my_server.py"), namespace="local")

github_config = {
    "mcpServers": {
        "default": {
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-github"]
        }
    }
}
mcp.mount(create_proxy(github_config), namespace="github")
```

This exact set of patterns—remote HTTP, local script, npm config—is documented. ([FastMCP][21])

### 14.20 Conflict resolution and precedence in composition

Conflict resolution differs slightly depending on the layer you are looking at. Provider order says `LocalProvider` wins over later-added providers. Within mounted-server composition specifically, the composition docs state that when multiple mounted servers share the same namespace (or no namespace), the **most recently mounted server** takes precedence for conflicting component names. That is a mount-layer override rule on top of the broader provider-order model. ([FastMCP][3])

The safe operational default is therefore:

* local overrides first;
* namespaced mounts to avoid accidental collision;
* if you intentionally share a namespace, assume latest mount wins and document it. ([FastMCP][3])

### 14.21 Recursive tag filtering and parent policy

The composition docs explicitly state that parent-server tag filters apply recursively to mounted servers. This means a parent can mount a large child and then publish only a filtered slice of it through tag-based visibility. Example patterns in the docs show mounting and then calling `enable(tags={"production"}, only=True)` on the parent so only namespaced production-tagged child components are exposed. ([FastMCP][21])

This is a powerful separation-of-concerns pattern: children publish full catalogs; the parent decides what slice of those catalogs is actually visible in a given orchestration or deployment context. It reinforces the earlier rule that providers source components while transforms/visibility policies decide publication. ([FastMCP][4])

### 14.22 Performance characteristics of composition

The composition docs explicitly warn that parent operations like `list_tools()` inherit the latency of all mounted sources. They call out three particularly expensive cases: HTTP-based mounted servers (hundreds of milliseconds instead of local-microseconds/milliseconds), mounted servers with slow initialization, and deep mounting hierarchies. The docs recommend caching strategies or shallower hierarchies when low latency is critical. ([FastMCP][21])

That performance note should heavily influence design: use mount/proxy composition for correctness, modularity, and operational safety—but do not assume the parent catalog is “free” once multiple remote or deep child sources are involved. Search transforms, cache layers, and explicit namespace partitioning become much more valuable in those topologies. ([FastMCP][21])

### 14.23 Provider vs middleware: the boundary

The custom-provider docs make this boundary unusually explicit: it is often cleaner to let a provider source all possible components and then use middleware or visibility controls to decide what a particular request can see. That is the crisp separation:

* provider = source all potential components
* transform/visibility = rewrite/filter the published catalog
* middleware = enforce per-request behavioral policy at execution time. ([FastMCP][4])

If an agent tries to express per-request auth policy in a provider, or source-discovery logic in middleware, the design usually becomes harder to reason about. FastMCP’s architecture is specifically trying to prevent that tangle. ([FastMCP][4])

### 14.24 Composition advisories for agent authors

Use `LocalProvider` for code-defined components, `mount(child_server)` for in-process modular composition, `create_proxy(...)` / `ProxyProvider` for remote or subprocess backends, `FileSystemProvider` when you want file-system discovery without server/file coupling, `SkillsDirectoryProvider` when the thing you are publishing is fundamentally a skill/resource corpus, and a custom `Provider` when the source is a database, API, config registry, or tenant-aware dynamic catalog. That mapping is the cleanest “which source layer should I pick?” heuristic supported by the docs. ([FastMCP][1])

Default to **namespaces** whenever you compose more than one nontrivial provider or child server. Use `ToolTransform` to improve ergonomics or schema clarity of mounted/proxied components instead of copying them locally. Use `wrap_transform()` instead of mutating shared provider instances when the same source will be mounted into multiple parents. ([FastMCP][15])

Use `ResourcesAsTools` and `PromptsAsTools` only on an actual server instance and preferably on a dedicated subserver when you need subset exposure. Do not apply them blindly at the top of a large server unless you actually need tool-only bridging for the entire resource/prompt catalog. The docs explicitly recommend dedicated subservers when only subsets should be exposed. ([FastMCP][19])

For remote composition, prefer `create_proxy()` over lower-level proxy classes unless you specifically need custom session factories or advanced proxy-provider control. Leave proxy session isolation enabled unless you fully understand the concurrency consequences of shared-session reuse. Disable proxy component-list caching when the upstream component catalog changes dynamically. ([FastMCP][8])

If you want the next section, the natural continuation is **13) Authentication and authorization**, because providers/transforms determine what exists and how it is published, while auth determines who can see or execute any of it.

[1]: https://gofastmcp.com/servers/providers/overview "Providers - FastMCP"
[2]: https://gofastmcp.com/servers/transforms/transforms "Transforms Overview - FastMCP"
[3]: https://gofastmcp.com/servers/providers/overview "Providers"
[4]: https://gofastmcp.com/servers/providers/custom "Custom Providers - FastMCP"
[5]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-fastmcp_provider "fastmcp_provider - FastMCP"
[6]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-aggregate "aggregate - FastMCP"
[7]: https://gofastmcp.com/servers/providers/proxy "MCP Proxy Provider - FastMCP"
[8]: https://gofastmcp.com/python-sdk/fastmcp-server-server "server - FastMCP"
[9]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-proxy "proxy - FastMCP"
[10]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-filesystem "filesystem - FastMCP"
[11]: https://gofastmcp.com/servers/providers/filesystem "Filesystem Provider"
[12]: https://gofastmcp.com/servers/providers/filesystem "Filesystem Provider - FastMCP"
[13]: https://gofastmcp.com/servers/providers/skills "Skills Provider - FastMCP"
[14]: https://gofastmcp.com/python-sdk/fastmcp-server-providers-base "base - FastMCP"
[15]: https://gofastmcp.com/servers/transforms/namespace "Namespace Transform - FastMCP"
[16]: https://gofastmcp.com/servers/transforms/tool-transformation "Tool Transformation - FastMCP"
[17]: https://gofastmcp.com/python-sdk/fastmcp-tools-tool_transform "tool_transform"
[18]: https://gofastmcp.com/servers/transforms/tool-search "Tool Search - FastMCP"
[19]: https://gofastmcp.com/servers/transforms/resources-as-tools "Resources as Tools - FastMCP"
[20]: https://gofastmcp.com/servers/transforms/prompts-as-tools "Prompts as Tools - FastMCP"
[21]: https://gofastmcp.com/servers/composition "Composing Servers - FastMCP"

# FastMCP Advanced — 15) Transforms, visibility, versioning, pagination, and discovery shaping

## 15.0 Publication-pipeline mental model

Providers answer **what components exist**. Transforms answer **what the published component graph should look like**. Final FastMCP filtering then decides what the current client/session is allowed to see.

```text
source Provider
   -> provider transforms
   -> aggregate providers
   -> server transforms
   -> enabled/disabled + authorization + session visibility
   -> pagination
   -> MCP list response
```

Transforms are therefore part of the public API contract. A rename, namespace, hidden argument, search transform, or visibility rule can change what a client sees without modifying the underlying Python function. ([Transforms][1])

---

## 15.1 Provider-level vs server-level transforms

Provider-local transform:

```python
provider = provider.wrap_transform(Namespace("billing"))
```

Server-level transform:

```python
mcp = FastMCP(
    "Gateway",
    providers=[provider],
    transforms=[global_transform],
)
```

Rules:

* provider transforms affect one source and run before aggregation;
* server transforms affect the combined server view and run later;
* later transforms see the output of earlier transforms;
* place a rule at the narrowest level that matches its intended scope. ([Transforms][1])

---

## 15.2 Namespace transform

Namespacing is the preferred collision-avoidance mechanism when composing component sources.

```python
from fastmcp.server.transforms import Namespace

mcp = FastMCP(
    "Gateway",
    providers=[
        crm_provider.wrap_transform(Namespace("crm")),
        billing_provider.wrap_transform(Namespace("billing")),
    ],
)
```

Conceptually:

```text
get_customer       -> crm_get_customer
create_invoice     -> billing_create_invoice
```

Resources/templates are namespaced according to their URI semantics rather than merely string-prefixing a tool name. ([Namespace][2])

---

## 15.3 `mount(..., namespace=...)`

For mounted FastMCP servers, use the mount API rather than manually cloning components:

```python
root = FastMCP("Root")
root.mount(crm, namespace="crm")
root.mount(billing, namespace="billing")
```

`mount()` constructs live provider composition; changes in the child server remain dynamically visible through the parent. Namespace transforms make collisions explicit. ([Composition][3])

---

## 15.4 Tool transformation

Tool transformations let you adapt a tool for LLM use without rewriting its implementation:

```text
original tool
  -> rename tool
  -> rename/hide arguments
  -> rewrite descriptions
  -> change schema presentation
  -> published transformed tool
```

Use cases:

* normalize a third-party API tool name;
* replace implementation-oriented parameter names with model-friendly names;
* hide infrastructure parameters;
* narrow descriptions;
* adapt an OpenAPI-generated tool into a curated contract.

Prefer transformation when the underlying callable remains a valid reusable implementation and only the MCP-facing interface needs to change. ([Tool Transformation][4])

---

## 15.5 Transform vs wrapper tool

Choose a transform when:

```text
behavior stays the same
input/output semantics stay fundamentally the same
only publication shape changes
```

Choose a wrapper tool when:

```text
behavior changes
multiple backend operations are composed
a policy decision must run in business logic
input/output semantics are intentionally redesigned
```

A transform is not a substitute for domain orchestration.

---

## 15.6 Resources-as-tools bridge

Some MCP hosts primarily expose tool calling. `ResourcesAsTools` bridges the resource surface into synthetic tools for listing and reading resources.

Conceptual result:

```text
resource protocol surface
   -> list_resources tool
   -> read_resource tool
```

Use only when client compatibility requires it; do not turn every resource into a bespoke imperative tool by hand. ([Resources as Tools][5])

---

## 15.7 Prompts-as-tools bridge

`PromptsAsTools` similarly adapts prompt listing/rendering for tool-only clients.

Use it when the consuming host lacks native prompt operations but the prompt catalog should remain authored as prompts on the server. This preserves the semantic source model while adapting the published interface. ([Prompts as Tools][6])

---

## 15.8 Visibility model

FastMCP v3 uses visibility transforms rather than per-component mutable `enabled` flags as the primary publication-control mechanism.

Examples:

```python
mcp.disable(tags={"internal"})
mcp.enable(tags={"public"}, only=True)
mcp.disable(keys={"tool:dangerous_delete@"})
```

Visibility can target dimensions such as:

* component keys;
* names/URIs;
* tags;
* versions/version specs;
* component type/kind;
* match-all criteria.

Later visibility rules can override earlier ones. ([Visibility][7])

---

## 15.9 Component keys

FastMCP component identity includes component kind, identifier, and version. Conceptual examples:

```text
tool:get_customer@
tool:get_customer@2.0
prompt:review@
resource:config://app@
```

Use component keys when exact identity matters—fingerprinting, targeted visibility, audits, or version-specific policy. Names alone may be ambiguous once multiple versions exist. ([Versioning][8])

---

## 15.10 Allowlist mode with `only=True`

A strong publication posture is to start with an allowlist:

```python
mcp.enable(tags={"public"}, only=True)
```

Then selectively add or subtract:

```python
mcp.enable(tags={"beta"})
mcp.disable(names={"delete_account"})
```

This is safer for large/generated provider catalogs than registering hundreds of operations and trying to remember every one that should be hidden.

---

## 15.11 Session-scoped visibility in FastMCP 3

Within a request/session, `Context` can change the component view for that session:

```python
@mcp.tool
async def enter_admin_mode(ctx: Context) -> str:
    await ctx.enable_components(tags={"admin"})
    return "Admin components enabled for this session"
```

Related operations include disabling and resetting session visibility. This lets one server expose different catalogs based on workflow/client state without globally mutating the server. ([Context][9])

**Security rule:** visibility is not automatically authorization. Sensitive components should still enforce authorization even if normally hidden.

---

## 15.12 Versioned components

Register multiple implementations under the same public name:

```python
@mcp.tool(name="search", version="1.0")
def search_v1(query: str) -> list[str]:
    ...


@mcp.tool(name="search", version="2.0")
def search_v2(query: str, limit: int = 20) -> list[dict]:
    ...
```

When no version is requested explicitly, FastMCP selects the highest enabled version according to its version comparison rules. ([Versioning][8])

---

## 15.13 Version ordering

FastMCP attempts semantic/PEP-440-style comparison where possible and falls back for nonstandard strings. Prefer predictable version identifiers:

```text
1.0
1.1
2.0
2.1rc1
```

Avoid arbitrary labels whose lexical ordering surprises you:

```text
new
newer
final2
2026-special
```

If date versions are used, use a sortable canonical format such as `2026.08.19` only after verifying your exact version-comparison semantics.

---

## 15.14 Highest-version fallback and visibility

A key v3 behavior is that the highest version can be disabled while a lower enabled version remains available.

Conceptual:

```text
search@2.0 -> disabled for this client
search@1.0 -> enabled
get_tool("search") -> returns highest enabled version
```

This makes versioning useful for staged rollout/rollback without renaming public capabilities.

---

## 15.15 Versioning is not database migration

Component versions solve **MCP contract selection**. They do not solve:

* persistent state migration;
* DB schema migration;
* backward-compatible side effects;
* data-format conversion;
* cross-service API coordination.

Treat component version as one layer of a broader compatibility strategy.

---

## 15.16 Pagination

FastMCP v3 supports pagination for potentially large component lists. Configure the server page size:

```python
mcp = FastMCP(
    "Registry",
    list_page_size=50,
)
```

Pagination applies to protocol listing surfaces such as tools/resources/prompts where relevant. `None` means unpaginated. FastMCP requires positive page sizes. ([Pagination][10])

---

## 15.17 Why pagination matters even with Tool Search

Pagination and tool search solve different problems:

| Problem | Mechanism |
|---|---|
| protocol response too large | pagination |
| model should not receive full catalog | search transform / Code Mode |
| client needs all items eventually | paginate through pages |
| tool selection degrades with hundreds of tools | search transform |

Do not assume pagination alone fixes LLM context overload—the client may still collect every page and inject every tool into model context.

---

## 15.18 Search transforms as discovery shaping

Tool Search replaces a broad visible catalog with a small synthetic discovery interface while preserving the underlying tools as callable. FastMCP 3.1 made this a major capability. Detailed search/Code Mode behavior is in §16. ([Tool Search][11])

---

## 15.19 Transform order is API behavior

Bad mental model:

```text
"all transforms are independent decorations"
```

Correct mental model:

```text
Transform A changes a component
Transform B receives A's output
Transform C receives B's output
```

Example consequence:

```text
Namespace first -> later name filter must match namespaced names
Filter first    -> namespace sees only surviving components
```

Document transform order in production systems; do not rely on a future maintainer guessing it from a long constructor argument list.

---

## 15.20 Provider reuse and immutable wrapping

When the same provider will be mounted in multiple places with different transforms:

```python
public = base_provider.wrap_transform(public_transform)
admin = base_provider.wrap_transform(admin_transform)
```

Prefer immutable wrapping over mutating the shared provider with `add_transform` unless the provider instance is intentionally single-owner. This avoids transform leakage between servers. ([Providers][12])

---

## 15.21 Large generated provider pattern

For OpenAPI or dynamic registries:

```text
OpenAPIProvider / custom Provider
  -> source-specific ToolTransform (rename/clean descriptions)
  -> visibility allowlist
  -> Namespace if aggregated
  -> Tool Search / Code Mode if catalog is large
  -> server auth/authorization
```

This is preferable to exposing the raw generated endpoint catalog unchanged.

---

## 15.22 Publication policy vs runtime authorization

Use transforms/visibility for:

```text
what the model/client should normally see
```

Use authorization for:

```text
what the authenticated caller may execute/read/render
```

A hidden tool may still be callable by exact name depending on transform semantics; a secure tool must enforce access at resolution/execution time. See §17–18.

---

## 15.23 Version-aware rollout example

```python
@mcp.tool(name="summarize", version="1.0", tags={"stable"})
def summarize_v1(text: str) -> str:
    ...


@mcp.tool(name="summarize", version="2.0", tags={"beta"})
def summarize_v2(text: str, style: str = "concise") -> dict:
    ...

# default server posture
mcp.enable(tags={"stable"}, only=True)
```

A controlled session/user group can then enable `beta` components through an authorization-aware workflow. Avoid enabling beta merely from an untrusted client-supplied flag.

---

## 15.24 Contract-regression implication

A tool's Python function may be unchanged while its published MCP contract changes because of:

* transform configuration;
* namespace order;
* visibility rules;
* version selection;
* schema dereferencing policy;
* OpenAPI provider changes.

Therefore snapshot/fingerprint the **resolved client-visible tool**, not only the source function signature. See §30.

---

## 15.25 Anti-pattern inventory

* relying on provider registration order to resolve name collisions instead of namespaces;
* globally mutating a provider reused by several servers;
* treating hidden as authorized;
* exposing thousands of generated tools because pagination technically works;
* using `enabled=` on components as the main v3 control rather than visibility rules;
* mixing semantic tool versions with arbitrary deployment build IDs;
* assuming version strings always sort lexically;
* filtering before/after namespace transforms without documenting which names the filter sees;
* allowing a session to enable privileged components without authorization checks;
* hand-copying transformed tools into a second local registry;
* snapshotting source schemas but not resolved/transformed schemas.

---

## 15.26 Agent checklist

```text
[ ] Place transforms at provider or server scope deliberately.
[ ] Namespace independent providers before collisions happen.
[ ] Use transforms for publication shape; wrappers for behavioral changes.
[ ] Use visibility for discoverability/exposure, not as the sole auth boundary.
[ ] Use component keys for exact identity.
[ ] Use stable version strings and test highest-version selection.
[ ] Configure pagination for large catalogs.
[ ] Add Tool Search/Code Mode when model context would otherwise contain a huge catalog.
[ ] Use wrap_transform() for providers reused in multiple aggregators.
[ ] Snapshot/fingerprint final client-visible schemas after transforms.
```

[1]: https://gofastmcp.com/servers/transforms/transforms "Transforms"
[2]: https://gofastmcp.com/servers/transforms/namespace "Namespace Transform"
[3]: https://gofastmcp.com/servers/composition "Composing Servers"
[4]: https://gofastmcp.com/servers/transforms/tool-transformation "Tool Transformation"
[5]: https://gofastmcp.com/servers/transforms/resources-as-tools "Resources as Tools"
[6]: https://gofastmcp.com/servers/transforms/prompts-as-tools "Prompts as Tools"
[7]: https://gofastmcp.com/servers/visibility "Component Visibility"
[8]: https://gofastmcp.com/servers/versioning "Versioning"
[9]: https://gofastmcp.com/servers/context "MCP Context"
[10]: https://gofastmcp.com/servers/pagination "Pagination"
[11]: https://gofastmcp.com/servers/transforms/tool-search "Tool Search"
[12]: https://gofastmcp.com/servers/providers/overview "Providers"

# FastMCP Advanced — 16) Search transforms, Code Mode, composition, proxying, and gateways

## 16.0 Why this is a separate architecture topic

FastMCP 3.1 introduced discovery transforms aimed at a problem that ordinary MCP listing does not solve well: a server may legitimately own hundreds or thousands of tools, but sending every schema to an LLM wastes context and often makes selection worse. FastMCP's search and Code Mode surfaces treat tool discovery as an explicit runtime operation rather than an unconditional prompt payload. ([Tool Search][1]) ([FastMCP Updates][2])

At the same time, v3 providers make it easy to aggregate multiple servers, which increases catalog size further. The scalable pattern is therefore:

```text
many providers / mounted servers / proxies
    -> namespacing + curation
    -> search or Code Mode
    -> small model-visible discovery interface
```

---

## 16.1 Regex search transform

FastMCP's tool-search surface includes a zero-index-overhead regex strategy:

```python
from fastmcp import FastMCP
from fastmcp.server.transforms.search import RegexSearchTransform

mcp = FastMCP(
    "Registry",
    transforms=[RegexSearchTransform()],
)
```

Instead of exposing the entire catalog in `list_tools`, the transform publishes synthetic discovery/execution tools. The underlying tools remain callable; search changes discovery, not the existence of the original tool. ([Tool Search][1])

Use regex search when tool names/descriptions are already descriptive and the model can formulate useful lexical patterns.

---

## 16.2 Search-result contract

A search transform should give the model enough information to decide and invoke without forcing the full registry into context.

Conceptually:

```text
search_tools(pattern="invoice|billing")
    -> selected tool definitions + input schemas

call_tool(name="billing_create_invoice", arguments={...})
    -> execute underlying tool
```

The exact synthetic tool names/parameters are part of the transform contract; snapshot them if your agent prompt depends on them.

---

## 16.3 Search is discoverability, not security

A tool hidden from `list_tools` by search can still be directly callable by exact identity depending on the transform. Therefore:

```text
Search transform -> limits model-visible discovery surface
Authorization    -> limits what caller may actually execute
```

Never rely on "the model cannot see it" as the security control for destructive or privileged tools.

---

## 16.4 Code Mode — stable 3.1 feature, experimental transform

FastMCP 3.1's Code Mode lets the model discover tools and write Python orchestration that calls them inside a sandbox. Install the optional extra:

```bash
uv add "fastmcp[code-mode]==3.4.7"
```

Configure:

```python
from fastmcp import FastMCP
from fastmcp.experimental.transforms.code_mode import CodeMode

mcp = FastMCP(
    "CodeModeServer",
    transforms=[CodeMode()],
)
```

The transform is explicitly described as experimental even though the overall interface is usable in v3. Treat discovery tool details as more volatile than core `@mcp.tool` APIs. ([Code Mode][3])

---

## 16.5 Default Code Mode workflow

The documented mental model is a three-stage interaction:

```text
1. search
   -> identify likely tools

2. get_schema
   -> retrieve parameter details only for selected tools

3. execute
   -> run sandboxed Python that calls tools and returns final value
```

Inside the sandbox, the model uses a controlled `call_tool(name, params)` primitive rather than arbitrary direct access to your application objects. This reduces both up-front schema tokens and intermediate LLM round-trips for multi-tool workflows. ([Code Mode][3])

---

## 16.6 Code Mode value case

Without Code Mode:

```text
LLM -> tool A -> result A -> LLM -> tool B -> result B -> LLM -> tool C
```

Every intermediate result passes through model context.

With Code Mode:

```text
LLM -> execute(code chaining A/B/C) -> final result
```

Intermediate values can remain inside the sandboxed program. This is especially valuable when tools return large objects that are only inputs to the next operation.

---

## 16.7 Code Mode is not arbitrary code execution permission

Treat the sandbox as a security boundary:

* restrict available globals/imports according to the implementation;
* expose capability-bearing operations only through approved tool calls;
* bound runtime, memory, output size, and loops;
* treat tool authorization as still mandatory;
* do not assume sandboxing protects against every downstream tool side effect;
* review Code Mode dependency/runtime changes on upgrade.

The strongest security model is still "the model may only cause side effects through explicitly authorized tools."

---

## 16.8 Discovery detail tradeoff

FastMCP Code Mode supports progressive tool detail. Conceptually:

| Detail | Model receives | Cost |
|---|---|---|
| brief | tool name + short description | low |
| detailed | parameters/types/required markers | medium |
| full | full JSON Schema | high |

Use the least detail sufficient to write a correct call. Large nested schemas can dominate context if every search result returns full detail.

---

## 16.9 Compose focused child servers

```python
from fastmcp import FastMCP

crm = FastMCP("CRM")
billing = FastMCP("Billing")
analytics = FastMCP("Analytics")

root = FastMCP("EnterpriseGateway")
root.mount(crm, namespace="crm")
root.mount(billing, namespace="billing")
root.mount(analytics, namespace="analytics")
```

`mount()` is live composition: the parent delegates to the child's provider rather than creating a one-time copied snapshot. ([Composition][4])

---

## 16.10 Mount vs import/copy

Prefer `mount()` when:

* child server remains a modular unit;
* child lifecycle/middleware/provider behavior matters;
* components may change dynamically;
* namespacing should be systematic.

Legacy copy/import semantics are useful only when you explicitly want a snapshot detached from future child changes. In v3, mount/provider composition is the default architectural primitive.

---

## 16.11 Proxy a remote MCP server

A `ProxyProvider` sources components through a FastMCP client connection to another server. The convenience server factory is `create_proxy()`.

Representative pattern:

```python
from fastmcp.server import create_proxy

proxy = create_proxy("https://upstream.example.com/mcp")
```

A proxy can translate transports—for example, local STDIO on one side and HTTP on the other—while preserving MCP operations. ([Proxy Provider][5])

---

## 16.12 Proxy use cases

```text
transport bridge
    local STDIO client -> FastMCP proxy -> remote HTTP backend

security gateway
    client -> authenticated/authorized proxy -> internal MCP server

aggregation
    client -> root FastMCP -> several ProxyProviders

stable endpoint
    client -> gateway URL -> moving/versioned upstream backends
```

Do not add a proxy merely because composition is possible; every proxy hop adds latency, failure modes, auth/token considerations, and observability requirements.

---

## 16.13 Proxy initialization correctness in 3.4

FastMCP 3.4 tightened proxy initialization: the proxy forwards initialization upstream so a missing backend, wrong endpoint, or failed authentication causes the proxy handshake to fail rather than presenting an apparently connected server with no components. ([FastMCP Updates][2])

Operational implication: health checks should include upstream initialization, not only "the proxy process is alive."

---

## 16.14 `fastmcp-remote` vs a general proxy server

`fastmcp-remote` packages the common remote-HTTP → local-STDIO bridge as a dedicated executable:

```bash
uvx fastmcp-remote https://example.com/mcp
```

Use it when a desktop/host only knows how to launch STDIO but the actual MCP server is remote. Use a normal FastMCP proxy application when you need custom middleware, authorization, transforms, multiple backends, custom routes, or deployment logic. ([fastmcp-remote][6])

---

## 16.15 Namespace remote providers

When aggregating several remote servers, namespace every provider unless their contracts are deliberately coordinated.

Conceptual:

```python
root = FastMCP("Gateway")
root.mount(weather_proxy, namespace="weather")
root.mount(calendar_proxy, namespace="calendar")
```

This produces stable public identities regardless of coincidental backend tool names such as `search`, `get`, or `list`.

---

## 16.16 Gateway auth boundary

A gateway can authenticate the caller independently from upstream credentials:

```text
client credential
    -> gateway verifies caller
    -> gateway authorization policy
    -> gateway uses service/upstream token
    -> upstream MCP server
```

This is often preferable to forwarding the user's raw inbound `Authorization` header to every upstream. FastMCP 3.x includes explicit hardening to prevent credential/header leakage across proxied/OpenAPI backends; preserve that separation in custom middleware. See §18/32.

---

## 16.17 Gateway catalog curation

A good MCP gateway does more than concatenate tool lists.

Recommended pipeline:

```text
ProxyProvider A ─┐
ProxyProvider B ─┼─> namespace
LocalProvider ───┘      -> curate descriptions/schemas
                          -> visibility allowlist
                          -> version policy
                          -> search / Code Mode
                          -> auth/authorization
```

A raw union of several large enterprise tool catalogs is usually worse for model performance than a curated discovery layer.

---

## 16.18 Proxy failure taxonomy

| Failure | Where to detect |
|---|---|
| upstream DNS/connect failure | initialization / client transport |
| wrong MCP path | initialization |
| upstream auth failure | initialization/call depending provider flow |
| schema changed | contract/fingerprint test |
| one upstream slow | per-backend timeout/telemetry |
| proxy auth succeeds, upstream auth fails | gateway trace should distinguish both hops |
| tool name collision | composition build/namespace policy |
| output validation mismatch | proxy/provider result conversion |

Correlate request IDs/traces across gateway and upstream when possible.

---

## 16.19 Search over aggregated providers

Search should run after the naming/curation steps whose outputs the model is expected to invoke.

```text
wrong conceptual order:
search backend raw names -> rename/namespace later

better:
namespace/transform -> search final public names
```

Otherwise search results may teach the model a name that is not the callable public name.

---

## 16.20 Tool descriptions become search index data

When using search transforms, descriptions and parameter descriptions are not merely UI documentation—they are retrieval text. Improve retrieval by:

* using distinctive action/domain language;
* documenting important nouns/objects;
* avoiding dozens of tools with identical generic descriptions;
* including parameter descriptions that distinguish similar tools;
* keeping deprecated/internal terminology out of public descriptions.

Treat tool documentation as retrieval quality engineering.

---

## 16.21 Code Mode suitability matrix

| Catalog/workflow | Recommendation |
|---|---|
| 10 simple tools | normal MCP listing usually simplest |
| 100+ tools, mostly single calls | search transform |
| 100+ tools, common multi-step pipelines | evaluate Code Mode |
| sensitive destructive tools | Code Mode only with strong auth/tool policy |
| highly dynamic generated API | search + curation; Code Mode if orchestration valuable |
| tool results are huge intermediates | Code Mode can substantially reduce model context |

---

## 16.22 Testing search and Code Mode

Test:

```text
search retrieves expected tools for domain terms
search does not expose disallowed tools
returned tool names are actually callable
namespaced names match search output
schema detail is sufficient for valid calls
Code Mode sandbox cannot bypass tool authorization
execution timeout/output limits work
multi-tool orchestration returns only intended final output
```

Do not evaluate only with one hand-picked prompt; use a query set representative of real tool-selection intents.

---

## 16.23 Anti-pattern inventory

* aggregating multiple servers without namespaces;
* publishing 1,000 tools directly because the protocol permits it;
* treating search-hidden tools as secure;
* enabling Code Mode without evaluating sandbox/tool side effects;
* passing raw inbound auth headers indiscriminately to proxied backends;
* health-checking only the gateway process and not upstream initialization;
* indexing raw backend names but publishing renamed/namespaced names;
* using a proxy where a direct client would be simpler and equally secure;
* duplicating child components into a parent rather than mounting providers;
* allowing one slow backend to consume the gateway's entire request budget.

---

## 16.24 Agent checklist

```text
[ ] Estimate catalog size before choosing normal listing vs search.
[ ] Namespace every independently owned provider/server.
[ ] Curate generated/remote schemas before search indexing.
[ ] Treat search as discovery only; enforce authorization separately.
[ ] Use Code Mode only after evaluating sandbox and tool side effects.
[ ] Prefer mount/provider composition over copied component snapshots.
[ ] Make proxy initialization verify upstream availability.
[ ] Separate caller credentials from upstream service credentials.
[ ] Add per-upstream timeouts/traces/health checks.
[ ] Test retrieval quality with representative user intents.
```

[1]: https://gofastmcp.com/servers/transforms/tool-search "Tool Search"
[2]: https://gofastmcp.com/updates "FastMCP Updates"
[3]: https://gofastmcp.com/servers/transforms/code-mode "Code Mode"
[4]: https://gofastmcp.com/servers/composition "Composing Servers"
[5]: https://gofastmcp.com/servers/providers/proxy "Proxy Provider"
[6]: https://gofastmcp.com/clients/fastmcp-remote "fastmcp-remote"

# FastMCP Advanced — 17) Authentication and authorization
### Authentication and authorization

### 17.0 Split the domain cleanly

In FastMCP, **authentication** answers “is this bearer token valid, and who issued it?” while **authorization** answers “given a validated token, which tools/resources/prompts may this caller see or execute?” The docs are explicit that when an `AuthProvider` is configured, unauthenticated MCP-endpoint requests are rejected at the transport boundary before authorization checks run; authorization then differentiates among authenticated callers based on scopes and claims. ([FastMCP][1])

Authentication is only meaningful on HTTP-based transports. The auth overview states it applies to FastMCP’s HTTP transports (`http` and `sse`), while the authorization docs phrase the same boundary as OAuth tokens being available only on SSE and Streamable HTTP; both agree that STDIO inherits security from the local execution environment and that auth checks are skipped there because there is no OAuth concept. ([FastMCP][1])

At deployment time, auth providers also own the server’s discovery/operational auth surface. The auth SDK documents `get_routes(mcp_path)` and `get_middleware()` on auth providers; `TokenVerifier` typically contributes no auth routes, `RemoteAuthProvider` contributes protected-resource metadata routes, and `OAuthProvider` contributes full authorization-server routes. ([FastMCP][2])

---

## 17.1 Authentication: choose the right provider family

### 17.1.1 `TokenVerifier`: pure token validation, no OAuth discovery

Use a bare `TokenVerifier` when clients already know how to obtain tokens and your server only needs to validate them. The token-verification docs define this as the “pure resource server” pattern: token issuance happens elsewhere, FastMCP validates signatures/claims and makes access decisions locally. This is best for internal systems, API-gateway-backed services, or environments where client token acquisition is already solved outside MCP discovery. ([FastMCP][3])

FastMCP documents multiple verifier families under this pattern. `JWTVerifier` supports JWKS-based validation against an issuer and audience, static public keys, and HMAC/shared-secret validation for internal systems; `IntrospectionTokenVerifier` supports RFC 7662 opaque-token introspection; `StaticTokenVerifier` is explicitly development/testing-only; and `DebugTokenVerifier` exists for flexible test/prototyping scenarios. ([FastMCP][3])

```python id="9zt3vb"
from fastmcp import FastMCP
from fastmcp.server.auth.providers.jwt import JWTVerifier

auth = JWTVerifier(
    jwks_uri="https://auth.example.com/.well-known/jwks.json",
    issuer="https://auth.example.com",
    audience="mcp-api",
)

mcp = FastMCP("Protected API", auth=auth)
```

That pattern validates bearer tokens but does not expose OAuth discovery metadata for automated MCP client registration/login flows. It is therefore the right choice only when token acquisition is already handled elsewhere or preconfigured on clients. ([FastMCP][3])

### 17.1.2 `RemoteAuthProvider`: DCR-capable external IdP integration

Use `RemoteAuthProvider` when the upstream identity provider supports **Dynamic Client Registration (DCR)**. The remote-oauth docs define this as the fully automated MCP-native path: clients discover auth requirements, dynamically register, and obtain credentials without manual console setup. `RemoteAuthProvider` composes a `TokenVerifier` with trusted authorization-server metadata and emits standardized OAuth protected-resource discovery endpoints. ([FastMCP][4])

The docs are explicit that `RemoteAuthProvider` is verifier-agnostic: pair it with `JWTVerifier` for self-contained tokens, `IntrospectionTokenVerifier` for RFC 7662 opaque tokens, or a custom verifier if needed. The key requirement is not token format but DCR support at the identity provider. ([FastMCP][4])

```python id="yi47gv"
from pydantic import AnyHttpUrl
from fastmcp import FastMCP
from fastmcp.server.auth import RemoteAuthProvider
from fastmcp.server.auth.providers.jwt import JWTVerifier

token_verifier = JWTVerifier(
    jwks_uri="https://auth.example.com/.well-known/jwks.json",
    issuer="https://auth.example.com",
    audience="mcp-api",
)

auth = RemoteAuthProvider(
    token_verifier=token_verifier,
    authorization_servers=[AnyHttpUrl("https://auth.example.com")],
    base_url="https://api.example.com",
    allowed_client_redirect_uris=["http://localhost:*", "http://127.0.0.1:*"],
)

mcp = FastMCP("Company API", auth=auth)
```

The high-value parameters here are `token_verifier` for local token validation, `authorization_servers` to advertise which issuers the resource server trusts, `base_url` to identify the MCP server in OAuth metadata, and optional client redirect-URI restrictions for tighter registration policy. ([FastMCP][5])

### 17.1.3 `OAuthProxy`: bridge non-DCR OAuth providers

Use `OAuthProxy` when the upstream provider **does not** support DCR. The OAuth-proxy docs state this covers most traditional OAuth providers and enterprise identity systems, including GitHub, Google, Azure, AWS, Discord, and similar providers requiring manual app registration and fixed credentials. `OAuthProxy` presents a DCR-compliant surface to MCP clients while using your pre-registered upstream client ID/secret behind the scenes. ([FastMCP][6])

Its core value is callback forwarding. MCP clients often use dynamic localhost redirect URIs or fixed client-hosted callbacks, while traditional providers want one fixed redirect URI registered in advance. `OAuthProxy` stores the client’s dynamic callback URI, performs upstream auth using the server’s fixed callback, then forwards the result back to the client. The docs also say PKCE forwarding is enabled by default, and client redirect URIs can be restricted with wildcard-capable allowlists. ([FastMCP][6])

```python id="7zctv6"
from fastmcp import FastMCP
from fastmcp.server.auth import OAuthProxy
from fastmcp.server.auth.providers.jwt import JWTVerifier

token_verifier = JWTVerifier(
    jwks_uri="https://accounts.example.com/.well-known/jwks.json",
    issuer="https://accounts.example.com",
    audience="my-upstream-app-id",
)

auth = OAuthProxy(
    upstream_authorization_endpoint="https://accounts.example.com/oauth/authorize",
    upstream_token_endpoint="https://accounts.example.com/oauth/token",
    upstream_client_id="client-id",
    upstream_client_secret="client-secret",
    token_verifier=token_verifier,
    base_url="https://mcp.example.com",
    allowed_client_redirect_uris=[
        "http://localhost:*",
        "http://127.0.0.1:*",
        "https://*.example.com/*",
    ],
)

mcp = FastMCP("Proxy-Protected Server", auth=auth)
```

The key constructor fields are the upstream authorization/token endpoints, your upstream app credentials, a `TokenVerifier` for the returned tokens, `base_url`, optional `redirect_path`, optional `issuer_url`, and optional allowed client redirect-URI patterns. The docs also emphasize that the provider-console redirect URI must exactly match `base_url + redirect_path`. ([FastMCP][6])

### 17.1.4 `OIDCProxy`: OAuthProxy plus OIDC discovery

Use `OIDCProxy` when the upstream provider exposes a standard OpenID Connect discovery document (`/.well-known/openid-configuration`) but still is not a clean DCR fit for MCP. The OIDC-proxy docs describe it as being built on `OAuthProxy` while automatically discovering endpoints from the provider’s OIDC configuration, which reduces manual endpoint wiring and is a better fit for Auth0/Google/Azure/AWS-style providers with OIDC metadata. ([FastMCP][7])

```python id="tr7gxy"
from fastmcp import FastMCP
from fastmcp.server.auth.oidc_proxy import OIDCProxy

auth = OIDCProxy(
    config_url="https://login.example.com/.well-known/openid-configuration",
    client_id="client-id",
    client_secret="client-secret",
    base_url="https://mcp.example.com",
    audience="https://api.example.com",  # for providers that require an API audience
)

mcp = FastMCP("OIDC-Protected Server", auth=auth)
```

The high-value OIDC-specific parameters are `config_url`, `audience`, `strict`, `timeout_seconds`, and optional `token_verifier` override. If you already know you need a custom verifier, supply it directly; otherwise OIDCProxy can synthesize a default `JWTVerifier` from discovery metadata. ([FastMCP][7])

### 17.1.5 `OAuthProvider`: full in-server authorization server

Use `OAuthProvider` only when your MCP server must itself become the authorization server. The full-oauth docs are unusually direct: this is an extremely advanced pattern most users should avoid, because secure OAuth 2.1 implementation requires deep protocol, cryptography, storage, and operational-security expertise. The docs recommend external identity providers unless you have requirements they cannot meet, such as air-gapped or specialized-compliance deployments. ([FastMCP][8])

`OAuthProvider` is abstract. You must subclass it and implement client management, authorization flow, and token-management methods such as `get_client`, `register_client`, `authorize`, `load_authorization_code`, `exchange_authorization_code`, `load_refresh_token`, `exchange_refresh_token`, `load_access_token`, `revoke_token`, and `verify_token`. FastMCP supplies the protocol/route shell; you supply storage, user auth, consent, and token lifecycle logic. ([FastMCP][8])

```python id="ih3mru"
from fastmcp import FastMCP
from fastmcp.server.auth.providers.oauth import OAuthProvider

class MyOAuthProvider(OAuthProvider):
    async def get_client(self, client_id): ...
    async def register_client(self, client_info): ...
    async def authorize(self, client, params): ...
    async def load_authorization_code(self, client, authorization_code): ...
    async def exchange_authorization_code(self, client, authorization_code): ...
    async def load_refresh_token(self, client, refresh_token): ...
    async def exchange_refresh_token(self, client, refresh_token, scopes): ...
    async def load_access_token(self, token): ...
    async def revoke_token(self, token): ...
    async def verify_token(self, token): ...

mcp = FastMCP("Auth Server", auth=MyOAuthProvider(...))
```

Treat this as an explicit last-resort architecture, not a default. The docs emphasize that implementation complexity extends far beyond initial coding into ongoing security maintenance. ([FastMCP][8])

### 17.1.6 `MultiAuth`: multiple token sources, one server

Use `MultiAuth` when one FastMCP server must accept tokens from multiple sources. The docs define the canonical case as an interactive path authenticating through `OAuthProxy` or another auth server, plus machine-to-machine JWTs validated directly through one or more `TokenVerifier`s. `MultiAuth` composes them into one `auth=` object. ([FastMCP][9])

Verification order is deterministic: the auth server, if present, is tried first; additional verifiers are then tried in list order; the first successful verification wins. The server, if present, owns OAuth routes and metadata, while verifiers contribute only token-verification logic. This keeps discovery clean while expanding acceptable token sources. ([FastMCP][9])

```python id="u4v2pi"
from fastmcp import FastMCP
from fastmcp.server.auth import MultiAuth, OAuthProxy
from fastmcp.server.auth.providers.jwt import JWTVerifier

auth = MultiAuth(
    server=OAuthProxy(
        upstream_authorization_endpoint="https://login.example.com/oauth/authorize",
        upstream_token_endpoint="https://login.example.com/oauth/token",
        upstream_client_id="interactive-client",
        upstream_client_secret="interactive-secret",
        token_verifier=JWTVerifier(
            jwks_uri="https://login.example.com/.well-known/jwks.json",
            issuer="https://login.example.com",
            audience="interactive-client",
        ),
        base_url="https://mcp.example.com",
    ),
    verifiers=[
        JWTVerifier(
            jwks_uri="https://internal-issuer.example.com/.well-known/jwks.json",
            issuer="https://internal-issuer.example.com",
            audience="mcp.example.com",
        )
    ],
)

mcp = FastMCP("Hybrid Auth Server", auth=auth)
```

If you pass only verifiers and no server, `MultiAuth` still works, but no OAuth routes or discovery metadata are served. The docs recommend that shape for internal systems where clients already know how to obtain tokens. ([FastMCP][9])

### 17.1.7 Decision framework

Use **plain `TokenVerifier`** when clients already know token acquisition and you only need validation. Use **`RemoteAuthProvider`** when the upstream IdP supports DCR and you want MCP-native automated client registration. Use **`OIDCProxy`** when the upstream provider exposes OIDC discovery but does not cleanly support DCR for MCP. Use **`OAuthProxy`** when the upstream provider lacks DCR and you must bridge fixed upstream credentials to MCP’s dynamic client expectations. Use **`MultiAuth`** when more than one token source must be accepted. Use **`OAuthProvider`** only when you must issue and manage tokens yourself. This exact split is the combined guidance of the authentication overview, remote-oauth docs, OAuth-proxy docs, OIDC-proxy docs, and MultiAuth docs. ([FastMCP][1])

### 17.1.8 Deployment advisories for authentication

Configure auth providers programmatically via `auth=...` on `FastMCP(...)`, and keep secrets such as client secrets in environment variables rather than hardcoding them. The authentication overview explicitly recommends environment-based secret loading while keeping dependencies explicit in code. ([FastMCP][1])

If you proxy or mount the server under a path, remember that auth providers contribute discovery/operational routes, and path/base-URL correctness becomes part of the auth contract. This is especially important for `OAuthProxy`/`OIDCProxy`, where `base_url`, callback path, and optionally `issuer_url` govern discovery and redirect correctness. ([FastMCP][6])

---

## 17.2 Authorization: callable checks over authenticated requests

FastMCP authorization is callable-based. The authorization docs define an auth check as any sync or async callable receiving `AuthContext` and returning `True` or `False`; multiple checks combine with AND semantics. `AuthorizationError` may be raised for explicit denial messages, while other exceptions are masked for security and treated as denial. `AuthContext` exposes the current `token` and the `component` being accessed. ([FastMCP][10])

Because unauthenticated requests are already rejected at the transport boundary when an auth provider is configured, authorization logic is typically about **scope/claim/component-policy decisions**, not “is there a token at all?” The docs say this explicitly. In STDIO, there is no OAuth mechanism, so `get_access_token()` returns `None` and auth checks are skipped. ([FastMCP][10])

### 17.2.1 `require_scopes(...)`

`require_scopes(*scopes)` is the canonical scope gate. The authorization docs and SDK both state that it requires **all** specified scopes to be present on the token; multiple scopes inside one `require_scopes(...)` call are therefore AND-combined. ([FastMCP][10])

```python id="q4cbgz"
from fastmcp import FastMCP
from fastmcp.server.auth import require_scopes

mcp = FastMCP("Scoped Server")

@mcp.tool(auth=require_scopes("admin"))
def admin_operation() -> str:
    return "Admin action completed"

@mcp.tool(auth=require_scopes("read", "write"))
def read_write_operation() -> str:
    return "Read/write action completed"
```

Use `require_scopes(...)` for direct component ownership rules: “this operation always requires these scopes.” It is the most precise component-level authorization primitive FastMCP ships. ([FastMCP][10])

### 17.2.2 `restrict_tag(...)`

`restrict_tag(tag, scopes=[...])` is the conditional/tag-driven policy primitive. The authorization docs say it applies required scopes **only if** the component has the specified tag; untagged components are unaffected. This makes it the right primitive for server-wide or group-wide policy layered over a shared component catalog. ([FastMCP][10])

```python id="h7y7ib"
from fastmcp import FastMCP
from fastmcp.server.auth import restrict_tag
from fastmcp.server.middleware import AuthMiddleware

mcp = FastMCP(
    "Tagged Server",
    middleware=[
        AuthMiddleware(auth=restrict_tag("admin", scopes=["admin"]))
    ],
)

@mcp.tool(tags={"admin"})
def delete_all_data() -> str:
    return "Deleted"

@mcp.tool(tags={"public"})
def read_status() -> str:
    return "OK"
```

Use `restrict_tag(...)` when the same policy should apply to many components without repeating `auth=` on each one, or when mounted/proxied/server-composed components should inherit policy from shared tags. ([FastMCP][10])

### 17.2.3 AND-composition of checks

FastMCP combines checks with AND semantics in two places. First, passing a list to `auth=[...]` on a component means all checks must pass. Second, component-level `auth=` and server-level `AuthMiddleware(auth=...)` are both evaluated, and both must pass. The docs explicitly spell out both forms of AND-composition. ([FastMCP][10])

```python id="qj7ocg"
from fastmcp import FastMCP
from fastmcp.server.auth import require_scopes, restrict_tag
from fastmcp.server.middleware import AuthMiddleware

mcp = FastMCP(
    "Layered Auth Server",
    middleware=[AuthMiddleware(auth=restrict_tag("admin", scopes=["admin"]))],
)

@mcp.tool(auth=[require_scopes("write"), require_scopes("audit")], tags={"admin"})
def admin_write() -> str:
    return "Admin write"
```

The effective policy here is: caller must satisfy both component-level checks (`write` AND `audit`) and the middleware-enforced tag rule (`admin` for `admin`-tagged components). ([FastMCP][10])

### 17.2.4 Custom and async auth checks

Any callable `AuthContext -> bool` is a valid auth check, and the docs explicitly say checks may be asynchronous. This means authorization can inspect token claims, component metadata, or even external services/state before allowing access. The SDK’s `run_auth_checks(...)` supports sync and async check functions with the same AND semantics. ([FastMCP][10])

```python id="sq9mgo"
from fastmcp import FastMCP
from fastmcp.server.auth import AuthContext, AuthorizationError

mcp = FastMCP("CustomAuth")

def require_verified_email(ctx: AuthContext) -> bool:
    if ctx.token is None:
        raise AuthorizationError("Authentication required")
    if not ctx.token.claims.get("email_verified"):
        raise AuthorizationError("Email verification required")
    return True

@mcp.tool(auth=require_verified_email)
def sensitive_operation() -> str:
    return "Allowed"
```

Use custom checks when `require_scopes` and `restrict_tag` are too coarse. Typical reasons: claim-based org/tenant membership, component-metadata-driven policy, external ACL lookups, or temporal/business-policy checks. ([FastMCP][10])

### 17.2.5 Component-level authorization

Component-level authorization is set with the `auth=` parameter on `@mcp.tool`, `@mcp.resource`, and `@mcp.prompt`. The authorization docs say this controls **both** visibility and access: unauthorized components are filtered out of list responses, and direct access behaves as not-found for unauthorized requests. ([FastMCP][10])

```python id="63zrq4"
from fastmcp import FastMCP
from fastmcp.server.auth import require_scopes

mcp = FastMCP("ComponentAuth")

@mcp.tool(auth=require_scopes("write"))
def write_tool() -> str:
    return "Written"

@mcp.resource("secret://data", auth=require_scopes("read"))
def secret_resource() -> str:
    return "Secret data"

@mcp.prompt(auth=require_scopes("admin"))
def admin_prompt() -> str:
    return "Admin prompt content"
```

Use component-level auth when the rule is intrinsic to the component itself. This is the lowest-friction way to encode “this capability is only for X-scoped callers.” ([FastMCP][10])

### 17.2.6 Server-level authorization via `AuthMiddleware`

For global policy, use `AuthMiddleware`. The middleware SDK says it applies auth checks to all components, filters list responses, checks authorization before tool execution/resource reads/prompt renders, and skips all checks on STDIO. Its `auth=` parameter accepts a single check or a list, again with AND semantics. ([FastMCP][11])

```python id="6r2jlwm"
from fastmcp import FastMCP
from fastmcp.server.auth import require_scopes
from fastmcp.server.middleware import AuthMiddleware

mcp = FastMCP(
    "Enforced Auth Server",
    middleware=[AuthMiddleware(auth=require_scopes("api"))],
)

@mcp.tool
def any_tool() -> str:
    return "Protected"
```

The docs contrast this with component-level auth: middleware is for server-wide baseline policy; component auth is for per-component requirements. In practice, use middleware for coarse mandatory gates like “all operations require `api`” and component auth for finer capability-specific constraints. ([FastMCP][11])

### 17.2.7 Component auth + middleware auth

FastMCP treats component-level `auth` and `AuthMiddleware` as complementary layers. The authorization docs explicitly say both layers are checked and all checks must pass. This is the correct way to express “global minimum policy plus local stricter policy.” ([FastMCP][10])

That means the recommended layered pattern is:

* middleware for baseline rules (`api`, tenancy, tag gates, org-wide constraints)
* component `auth=` for component-specific scopes/claims. ([FastMCP][10])

### 17.2.8 Access-token-aware tools

Authorization answers “may this run?” but tools often need the token after authorization to personalize behavior, inspect claims, or call downstream services on behalf of the user. The authorization docs explicitly recommend `get_access_token()` for this, and the DI SDK documents `CurrentAccessToken()` as the stricter injected form that raises when no authenticated user exists. ([FastMCP][12])

Optional access pattern:

```python id="jlwmzm"
from fastmcp import FastMCP
from fastmcp.server.dependencies import get_access_token

mcp = FastMCP("TokenAware")

@mcp.tool
def personalized_greeting() -> str:
    token = get_access_token()
    if token is None:
        return "Hello, guest!"
    name = token.claims.get("name", "user")
    return f"Hello, {name}!"
```

Required-token DI pattern:

```python id="4ewrq3"
from fastmcp import FastMCP
from fastmcp.dependencies import CurrentAccessToken
from fastmcp.server.auth import AccessToken

mcp = FastMCP("StrictTokenAware")

@mcp.tool
def user_dashboard(token: AccessToken = CurrentAccessToken()) -> dict:
    return {
        "sub": token.claims.get("sub"),
        "scopes": token.scopes,
        "client_id": token.client_id,
    }
```

Use `get_access_token()` when behavior can degrade for anonymous/no-token contexts; use `CurrentAccessToken()` when the tool should fail immediately if no authenticated principal exists. The DI SDK is explicit that `CurrentAccessToken()` raises without authentication, while `get_access_token()` is the optional form. ([FastMCP][13])

### 17.2.9 Best-practice authorization advisory

Encode stable, intrinsic rules at the component with `auth=`; encode organization-wide baselines with `AuthMiddleware`; layer them deliberately when both are needed. Use scopes for hard capability gates, tags for broad policy classification, and custom checks only when claim/tag/scope patterns are insufficient. That guidance follows directly from FastMCP’s documented split between component-level auth, tag-restriction helpers, and middleware-wide checks. ([FastMCP][10])

Do not use authorization as a substitute for authentication. Once an `AuthProvider` is configured, unauthenticated requests are already blocked at the transport boundary; your authorization layer should assume it is evaluating authenticated principals and make decisions on scopes, claims, and component metadata. ([FastMCP][10])


[1]: https://gofastmcp.com/servers/auth/authentication "Authentication - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-server-auth-auth "auth - FastMCP"
[3]: https://gofastmcp.com/servers/auth/token-verification "Token Verification - FastMCP"
[4]: https://gofastmcp.com/servers/auth/remote-oauth "Remote OAuth - FastMCP"
[5]: https://gofastmcp.com/servers/auth/remote-oauth "Remote OAuth"
[6]: https://gofastmcp.com/servers/auth/oauth-proxy "OAuth Proxy - FastMCP"
[7]: https://gofastmcp.com/servers/auth/oidc-proxy "OIDC Proxy - FastMCP"
[8]: https://gofastmcp.com/servers/auth/full-oauth-server "Full OAuth Server - FastMCP"
[9]: https://gofastmcp.com/servers/auth/multi-auth "Multiple Auth Sources - FastMCP"
[10]: https://gofastmcp.com/servers/authorization "Authorization - FastMCP"
[11]: https://gofastmcp.com/python-sdk/fastmcp-server-middleware-authorization "authorization - FastMCP"
[12]: https://gofastmcp.com/servers/authorization "Authorization"
[13]: https://gofastmcp.com/python-sdk/fastmcp-server-dependencies "dependencies"

# FastMCP Advanced — 18) Advanced security policy and identity-aware execution

## 18.0 Security model: five boundaries

A production FastMCP server typically has at least five distinct security boundaries:

```text
1. transport boundary
   -> who can reach the MCP endpoint?

2. authentication boundary
   -> who is the caller?

3. authorization boundary
   -> what may this caller list/call/read/render?

4. capability boundary
   -> what can each tool/resource actually do downstream?

5. data/tenant boundary
   -> which records/secrets/accounts can that operation access?
```

FastMCP can directly help with the first three and with tool publication, but application code still owns domain-level authorization and data isolation. ([Authentication][1]) ([Authorization][2])

---

## 18.1 Token auth is an HTTP boundary

FastMCP's OAuth/token authentication applies to HTTP-based transports. STDIO is a local process/launcher trust boundary; do not assume `auth=` protects a subprocess connection in the same way.

```text
HTTP -> token verifier / OAuth provider -> access token -> auth checks
STDIO -> OS/process/host configuration trust boundary
```

If a local STDIO tool can perform privileged actions, secure the host config, executable path, filesystem permissions, and any local credentials it can access. ([Authentication][1])

---

## 18.2 Authentication is not authorization

Authentication answers:

```text
"This token belongs to subject X with claims/scopes Y"
```

Authorization answers:

```text
"Subject X may perform action Z on resource R in tenant T"
```

Do not write:

```python
if get_access_token() is not None:
    return sensitive_data
```

when the actual policy needs tenant/resource-level checks.

---

## 18.3 Component-level auth

Sensitive components can attach authorization checks directly:

```python
from fastmcp.server.auth import require_scopes


@mcp.tool(auth=require_scopes("orders:write"))
async def create_order(...):
    ...
```

This has two benefits:

* unauthorized components can be filtered from catalog listing;
* exact-name/direct calls still enforce the same policy.

Use component-level policy for capability-specific checks; use middleware/server policy for broad cross-cutting rules. ([Authorization][2])

---

## 18.4 Access-token injection

Use FastMCP dependency injection to obtain the validated token/claims rather than reparsing raw HTTP headers in every tool.

Conceptual:

```python
from fastmcp.dependencies import CurrentAccessToken


@mcp.tool
async def who_am_i(token=CurrentAccessToken()) -> dict:
    return {
        "subject": token.claims.get("sub"),
        "scope": token.claims.get("scope"),
    }
```

The exact token type is part of the auth provider/SDK surface. Keep business logic behind a typed identity abstraction if many tools need the same claims. ([DI][3])

---

## 18.5 Build an application identity object

Prefer one dependency that validates/normalizes business identity:

```python
from dataclasses import dataclass
from fastmcp.dependencies import Depends


@dataclass(frozen=True)
class Principal:
    subject: str
    tenant_id: str
    roles: frozenset[str]


async def get_principal(token=CurrentAccessToken()) -> Principal:
    claims = token.claims
    return Principal(
        subject=claims["sub"],
        tenant_id=claims["tenant_id"],
        roles=frozenset(claims.get("roles", [])),
    )


@mcp.tool
async def get_invoice(
    invoice_id: str,
    principal: Principal = Depends(get_principal),
) -> dict:
    return await invoice_service.get_for_tenant(
        principal.tenant_id,
        invoice_id,
    )
```

This prevents every tool from implementing slightly different claim parsing.

---

## 18.6 Scope checks vs resource checks

A scope such as `invoices:read` is normally necessary but not sufficient for multi-tenant access.

```text
scope check:
    may call invoice-read capability

resource check:
    invoice belongs to caller's tenant / permitted account
```

Enforce both where applicable.

---

## 18.7 Visibility is not the security boundary

A component hidden by tag/visibility/search transforms may still exist in the server graph. Therefore:

```text
visibility -> model/client discoverability
component auth -> execution/read/render permission
```

Use visibility to reduce confusion and exposure; use auth to prevent access.

---

## 18.8 Token verifier invariants

For JWT/OIDC-style verification, configure and validate at least:

* expected issuer;
* expected audience/resource indicator;
* signature algorithm/key source;
* expiration/not-before semantics;
* required subject/tenant claims;
* scope/role claim format;
* acceptable clock skew;
* JWKS refresh/failure behavior.

Never validate only a JWT signature and assume every token signed by the issuer was intended for your MCP server.

---

## 18.9 Patch-level auth fixes matter

FastMCP 3.4.x shipped multiple authentication/security fixes, including:

* IPv6 transition-address handling in SSRF protections;
* Host/Origin validation for DNS rebinding hardening;
* OAuth redirect restrictions;
* JWKS handling of unsupported key types;
* Azure audience/scope compatibility;
* trusted proxy support for protected metadata/JWKS fetching;
* CIMD client assertion audience correction.

This is why production deployments should review patch release notes rather than treating `3.4.0` and `3.4.7` as security-equivalent. ([FastMCP Updates][4])

---

## 18.10 OAuth Proxy trust boundary

`OAuthProxy` can bridge clients to an upstream provider that does not implement every MCP-oriented OAuth behavior directly. Model the token paths explicitly:

```text
MCP client
  -> FastMCP authorization interface
  -> FastMCP-managed client/token state
  -> upstream identity provider
  -> upstream access/refresh token
  -> client-facing FastMCP token/session
```

Never log access/refresh tokens. Ensure token caches are partitioned by identity/client context and have bounded expiry. ([OAuth Proxy][5])

---

## 18.11 CIMD and client assertions

Client ID Metadata Documents (CIMD) can reduce reliance on Dynamic Client Registration in compatible flows. FastMCP 3.4.7 specifically fixed `private_key_jwt` audience validation for bare-origin OAuth Proxy deployments. If you use client assertions:

* validate the assertion audience against the exact advertised token endpoint;
* verify signature/key ownership;
* enforce `exp`/`iat`/replay protections;
* test reverse-proxy URL normalization;
* pin the patch release containing known audience fixes.

See client and auth chapters for API details. ([FastMCP 3.4.7][6])

---

## 18.12 Proxy credential separation

Gateway anti-pattern:

```text
receive Authorization: Bearer USER_TOKEN
forward same header to every upstream service
```

Better:

```text
validate USER_TOKEN at gateway
derive Principal
apply gateway policy
use backend-specific credential / delegated token only for intended upstream
```

FastMCP has shipped fixes specifically around avoiding inbound auth-header leakage to unrelated proxy/OpenAPI backends. Preserve that boundary in custom middleware. ([FastMCP Updates][4])

---

## 18.13 OpenAPI-generated tools and credentials

When an OpenAPI provider uses an `httpx` client with backend credentials, those credentials are server-side capabilities. Do not expose them as LLM tool arguments.

```text
LLM supplies: endpoint business parameters
server supplies: backend auth headers, base URL, TLS settings
```

If tenants use different upstream credentials, inject the appropriate client/credential through authenticated identity and avoid one global privileged client.

---

## 18.14 SSRF posture

Any feature that fetches remote URLs based on metadata or user/provider configuration can become an SSRF surface. Threats include:

* `127.0.0.1` / RFC1918 targets;
* link-local/cloud metadata endpoints;
* IPv6-mapped/private targets;
* DNS rebinding;
* redirects from public to private addresses;
* proxy bypass;
* URL parser discrepancies.

FastMCP 3.4.x hardened OAuth metadata/JWKS fetches and IPv6 transition forms; do not replace those utilities with naive `httpx.get(user_url)` logic in custom auth. ([FastMCP Updates][4])

---

## 18.15 Trusted corporate proxies

FastMCP 3.4.6 added trusted-proxy support for SSRF-protected OAuth metadata/JWKS fetches. Enterprise egress designs should fail closed if protected fetching requires a mandated proxy and that proxy is unavailable, rather than silently bypassing it with a direct connection. ([FastMCP Updates][4])

Operationally test:

```text
proxy configured and reachable
proxy configured but unavailable
custom CA trust chain
no proxy configured when policy requires one
upstream redirect behavior through proxy
```

---

## 18.16 DNS rebinding / Host / Origin

A localhost-bound HTTP MCP server can still be targeted through a malicious browser if Host/Origin handling is weak. FastMCP 3.4.3 added Host/Origin validation and 3.4.4 adjusted defaults for deployment compatibility.

Production rule:

* if you know the public host(s), configure allowed hosts;
* if browser origins are relevant, configure allowed origins;
* verify reverse proxy rewrites/preserved host behavior;
* test both accepted and rejected Host/Origin cases;
* do not assume binding to `127.0.0.1` alone eliminates DNS-rebinding risk.

See §20/32 for deployment configuration. ([FastMCP Updates][4])

---

## 18.17 Destructive tool policy

MCP annotations such as destructive/read-only hints improve client UX but are not authorization.

For destructive tools:

```text
1. authenticate caller
2. authorize capability/resource
3. validate input
4. optionally require workflow approval/elicitation
5. perform operation with idempotency/concurrency controls
6. audit outcome
```

Use an app Approval provider or explicit workflow state for human confirmation when appropriate, but keep server-side authorization mandatory.

---

## 18.18 Least-privilege tool design

Prefer:

```text
issue_refund(order_id, amount)
```

over:

```text
execute_sql(sql)
execute_shell(command)
http_request(method, url, headers, body)
```

General-purpose escape hatches make policy difficult to reason about. If they are unavoidable, constrain destination/action/identity/arguments and isolate them behind stronger authorization.

---

## 18.19 Sandboxed-agent capability boundary

For isolated agents, FastMCP can act as the boundary that keeps long-lived upstream credentials outside the sandbox:

```text
sandboxed agent
   -> short-lived scoped MCP token
   -> FastMCP server
   -> privileged internal systems
```

The sandbox gets capabilities, not root credentials. Prefer remote HTTP for this architecture because authentication, revocation, and audit are centralized. ([Sandboxed Agents][7])

---

## 18.20 Error masking and information disclosure

Internet-facing servers should generally avoid exposing arbitrary Python exception details. Use `mask_error_details` and intentional tool/domain errors to control what clients learn.

Do not leak:

* filesystem paths;
* database connection strings;
* upstream tokens;
* internal hostnames;
* stack traces;
* authorization rule implementation details.

Log full diagnostic context server-side with appropriate redaction/correlation IDs.

---

## 18.21 Audit model

For sensitive actions, capture at least:

```text
timestamp
request / trace ID
principal subject
principal tenant
public tool key + version
validated arguments or redacted digest
policy decision
backend target / resource ID
result class (success / denied / error)
latency
```

Do not blindly log raw tool arguments if they can contain PII, secrets, or large payloads.

---

## 18.22 Authorization tests

Minimum test matrix:

```text
no token -> rejected
invalid signature -> rejected
wrong issuer -> rejected
wrong audience -> rejected
expired token -> rejected
valid token / missing scope -> hidden or denied
valid scope / wrong tenant -> denied
valid tenant -> allowed
exact-name call of hidden privileged tool -> still denied
proxy path does not forward caller credential to unrelated backend
```

Also test list visibility separately from direct execution.

---

## 18.23 Security anti-pattern inventory

* equating authentication with authorization;
* using tags/visibility as the sole security boundary;
* forwarding inbound Authorization headers to every upstream;
* verifying JWT signature without issuer/audience;
* exposing backend API tokens as tool parameters;
* user-controlled arbitrary URL fetches in auth/tools with no SSRF policy;
* binding localhost and assuming DNS rebinding is impossible;
* letting destructive tool annotations stand in for enforcement;
* logging full access/refresh tokens or secrets;
* putting tenant IDs from tool arguments ahead of authenticated tenant claims;
* running broad shell/SQL/HTTP escape-hatch tools with ordinary user scopes;
* leaving a known auth/security patch behind because major/minor version did not change.

---

## 18.24 Security checklist

```text
[ ] Classify transport trust: STDIO vs HTTP.
[ ] Validate token signature + issuer + audience + expiry.
[ ] Normalize claims into a typed Principal dependency.
[ ] Enforce capability and resource/tenant authorization.
[ ] Attach component auth to sensitive components.
[ ] Treat visibility/search as discoverability, not enforcement.
[ ] Separate caller credentials from backend credentials.
[ ] Use FastMCP SSRF-safe auth fetching rather than custom naive URL retrieval.
[ ] Configure Host/Origin policy for HTTP deployment shape.
[ ] Pin security-relevant 3.4.x patch releases.
[ ] Mask internal errors in public deployments.
[ ] Audit sensitive tool calls with redaction.
[ ] Test direct-call attempts against hidden/unauthorized tools.
```

[1]: https://gofastmcp.com/servers/auth/authentication "Authentication"
[2]: https://gofastmcp.com/servers/authorization "Authorization"
[3]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection"
[4]: https://gofastmcp.com/updates "FastMCP Updates"
[5]: https://gofastmcp.com/servers/auth/oauth-proxy "OAuth Proxy"
[6]: https://github.com/PrefectHQ/fastmcp/releases/tag/v3.4.7 "FastMCP 3.4.7"
[7]: https://gofastmcp.com/deployment/sandboxed-agents "Sandboxed Agents"

# FastMCP Advanced — 19) Running and deploying servers
### Running and deploying servers

### 19.0 Runtime surface: what actually binds a server to the outside world

Server construction defines the MCP surface; runtime methods bind that surface to a transport. The primary entrypoints are `run()`, `run_async()`, `run_http_async()`, and `http_app()`. The SDK transport surface documents the accepted transport literals for the top-level runtime APIs as `"stdio"`, `"http"`, `"sse"`, and `"streamable-http"`, while the prose deployment docs describe the transport families as **STDIO**, **HTTP / Streamable HTTP**, and **SSE (legacy)**. For agent-authored code, the safest reading is: use `transport="http"` for the recommended Streamable HTTP mode, `transport="stdio"` for subprocess/local integrations, and `transport="sse"` only for backward compatibility. ([FastMCP][1])

`run()` is the primary runtime entrypoint for simple deployment. The running guide says it starts the server, blocks until stopped, and handles connection management; the same guide also states that `run()` is a synchronous wrapper around the async implementation, while `run_async()` is the correct choice inside an already-async context. The transport SDK confirms both methods share the same transport surface. ([FastMCP][2])

### 19.1 `mcp.run()` — primary entrypoint, blocking semantics, and `__main__`

The canonical local/server script shape is:

```python id="xej7vt"
from fastmcp import FastMCP

mcp = FastMCP(name="MyServer")

@mcp.tool
def hello(name: str) -> str:
    return f"Hello, {name}!"

if __name__ == "__main__":
    mcp.run()
```

This is not stylistic decoration. The running guide explicitly says `run()` should live under `if __name__ == "__main__":` for maximum compatibility, so the server starts only when the file is executed directly and not when the module is imported by another Python process. For LLM-generated server files, that guard should be treated as mandatory unless the server is intentionally CLI-only or embedded in another runtime. ([FastMCP][2])

The async equivalent is:

```python id="eejc6b"
import asyncio
from fastmcp import FastMCP

mcp = FastMCP(name="MyServer")

@mcp.tool
def hello(name: str) -> str:
    return f"Hello, {name}!"

async def main():
    await mcp.run_async(transport="http", port=8000)

if __name__ == "__main__":
    asyncio.run(main())
```

The docs are explicit: `run()` cannot be called from inside an async function because it creates its own event loop; use `run_async()` in async contexts, and use `run()` in synchronous contexts. Both accept the same transport arguments. ([FastMCP][2])

### 19.2 Transport selection: STDIO vs Streamable HTTP vs SSE

FastMCP’s human-facing docs divide runtime transport choice into three deployment classes: **STDIO** for local single-process/subprocess-style use, **HTTP / Streamable HTTP** for remote or multi-client network services, and **SSE** only for older clients that have not moved to Streamable HTTP. The SDK transport page complements that by showing the accepted runtime literals. ([FastMCP][2])

#### STDIO

STDIO is the default when you call `run()` with no transport. The docs say it communicates over standard input/output, that the client spawns a new server process per session, and that it is ideal for local development, Claude Desktop, command-line tools, and single-user applications. Architecturally: choose STDIO when the client owns process lifecycle and there is no need for a shared network-reachable endpoint. ([FastMCP][2])

```python id="8kg7gn"
if __name__ == "__main__":
    mcp.run()  # default: STDIO
```

#### HTTP / Streamable HTTP

The running guide says HTTP transport turns the server into a URL-addressable web service, uses the Streamable HTTP protocol, supports full bidirectional communication and streaming responses, and is the recommended choice for network-based deployments. Unlike STDIO, one HTTP server can handle multiple concurrent clients. The HTTP deployment guide further says remote HTTP is how you unlock centralized services, multi-client access, and cloud deployment. ([FastMCP][2])

```python id="conryv"
if __name__ == "__main__":
    mcp.run(transport="http", host="127.0.0.1", port=8000)
```

By default, that yields an MCP endpoint at `http://localhost:8000/mcp`; both `run()` and `http_app()` also accept a custom `path=` if you want a different route such as `/api/mcp/`. ([FastMCP][2])

#### SSE

SSE is still supported, but the running guide calls it the original HTTP-based transport, explicitly labels it legacy, states that it only supports server-to-client streaming, and recommends HTTP instead for all new projects. SSE exists for backward compatibility with older clients, not as a default design target. ([FastMCP][2])

```python id="tlku4f"
if __name__ == "__main__":
    mcp.run(transport="sse", host="127.0.0.1", port=8000)
```

#### Decision rule

A concise deployment rule follows directly from the docs: choose **STDIO** for local, client-spawned, single-user integrations; choose **HTTP / Streamable HTTP** for remote services, multiple clients, and production network deployment; choose **SSE** only when a required client still depends on it. The docs phrase this decision almost exactly that way. ([FastMCP][2])

### 19.3 Direct HTTP server vs ASGI deployment

FastMCP documents two HTTP deployment styles: direct HTTP via `run(transport="http", ...)`, and ASGI deployment via `http_app()`. The HTTP deployment guide states that the direct HTTP server is simpler and better for quick standalone deployments, while the ASGI path is better when you need multiple workers, custom middleware, or integration with existing infrastructure. ([FastMCP][3])

#### Direct HTTP: fastest path to a standalone service

```python id="wj1dhf"
from fastmcp import FastMCP

mcp = FastMCP("My Server")

@mcp.tool
def process_data(input: str) -> str:
    return f"Processed: {input}"

if __name__ == "__main__":
    mcp.run(transport="http", host="0.0.0.0", port=8000)
```

The docs position this as ideal for internal tools, development environments, and simple deployments where the MCP server is the only thing on the port and you do not need higher-order ASGI features. ([FastMCP][3])

#### ASGI: production-oriented deployment primitive

`http_app()` is the ASGI-export path. The transport SDK defines:

```python id="dffw41"
http_app(
    path: str | None = None,
    middleware: list[ASGIMiddleware] | None = None,
    json_response: bool | None = None,
    stateless_http: bool | None = None,
    transport: Literal["http", "streamable-http", "sse"] = "http",
    event_store: EventStore | None = None,
    retry_interval: int | None = None,
) -> StarletteWithLifespan
```

That makes `http_app()` the correct choice when FastMCP needs to become a standard ASGI sub-application, when Uvicorn/Gunicorn/Hypercorn should own process management, or when you need deployment-time control over middleware, workers, and reverse-proxy behavior. The HTTP deployment guide explicitly recommends it for advanced server options, multiple workers, and mounting into larger web applications. ([FastMCP][1])

Minimal ASGI export:

```python id="nl3nq7"
from fastmcp import FastMCP

mcp = FastMCP("My Server")

@mcp.tool
def process_data(input: str) -> str:
    return f"Processed: {input}"

app = mcp.http_app()
```

Run with Uvicorn:

```bash
uvicorn app:app --host 0.0.0.0 --port 8000
```

The docs say the default URL remains `http://localhost:8000/mcp`, and that this path is the better fit for production reliability, performance, multiple workers, custom middleware, and existing deployment pipelines. ([FastMCP][3])

### 19.4 `run_http_async()` and serve-time knobs

The transport SDK exposes a more explicit HTTP runtime surface through `run_http_async(...)`, with `transport`, `host`, `port`, `path`, `uvicorn_config`, `middleware`, `json_response`, and `stateless_http`. This is the “serve-time binding” counterpart to the constructor and is where transport-specific choices belong. ([FastMCP][1])

```python id="kuhb1v"
await mcp.run_http_async(
    transport="http",
    host="0.0.0.0",
    port=8000,
    path="/api/mcp/",
    stateless_http=True,
)
```

Two agent-useful implications follow from the SDK signature. First, runtime transport choice is orthogonal to server construction; do not push host/port/path back into `FastMCP(...)`. Second, `stateless_http`, ASGI `middleware`, and endpoint `path` are deployment-time concerns that should remain environment-specific unless the deployment shape is itself part of the product contract. ([FastMCP][1])

### 19.5 Path semantics: `/mcp/` defaults, custom paths, and mount composition

The HTTP deployment guide says the default MCP HTTP endpoint is `/mcp/`, and both `run()` and `http_app()` accept `path=` to change it, for example to `/api/mcp/`. ([FastMCP][3])

```python id="4es3km"
# Direct server
mcp.run(transport="http", host="0.0.0.0", port=8000, path="/api/mcp/")

# ASGI export
app = mcp.http_app(path="/api/mcp/")
```

That changes the externally reachable endpoint to `http://localhost:8000/api/mcp/`. Use this when the MCP endpoint path itself is part of the deployment contract. ([FastMCP][3])

When mounting into another ASGI app, path composition becomes two-level:

* **mount prefix** from the parent ASGI router
* **internal MCP path** from `http_app(path=...)`

The docs show both patterns:

1. `mcp.http_app(path="/mcp")` mounted at `/analytics` gives `/analytics/mcp/`.
2. `mcp.http_app(path="/")` mounted at `/mcp` gives `/mcp`. ([FastMCP][4])

That distinction matters because many incorrect generated deployments accidentally double-prefix the MCP route. ([FastMCP][3])

### 19.6 Starlette mounting

The HTTP deployment guide documents Starlette mounting directly:

```python id="jcv79k"
from fastmcp import FastMCP
from starlette.applications import Starlette
from starlette.routing import Mount

mcp = FastMCP("MyServer")

@mcp.tool
def analyze(data: str) -> dict:
    return {"result": f"Analyzed: {data}"}

mcp_app = mcp.http_app(path="/mcp")

app = Starlette(
    routes=[
        Mount("/mcp-server", app=mcp_app),
    ],
    lifespan=mcp_app.lifespan,
)
```

The resulting MCP endpoint is `/mcp-server/mcp/`. The same page is explicit that for Streamable HTTP, you must pass the FastMCP lifespan into the parent Starlette app because nested lifespans are not recognized; otherwise the session manager will not initialize correctly. This is a correctness requirement, not a convenience. ([FastMCP][3])

### 19.7 FastAPI mounting

The FastAPI integration guide gives the equivalent pattern:

```python id="2q42ow"
from fastmcp import FastMCP
from fastapi import FastAPI

mcp = FastMCP("Analytics Tools")

@mcp.tool
def analyze_pricing(category: str) -> dict:
    return {"category": category}

mcp_app = mcp.http_app(path="/mcp")
app = FastAPI(title="E-commerce API", lifespan=mcp_app.lifespan)
app.mount("/analytics", mcp_app)
```

The documented result is MCP at `/analytics/mcp/`. The same guide later gives the “path already owned by mount” variant:

```python id="sokuqt"
mcp_app = mcp.http_app(path="/")
app = FastAPI(lifespan=mcp_app.lifespan)
app.mount("/mcp", mcp_app)   # endpoint: /mcp
```

The guide is explicit that omitting `lifespan=mcp_app.lifespan` is incorrect because the session manager will not initialize. ([FastMCP][4])

If the FastAPI application already has its own lifespan, the lifespans page recommends `combine_lifespans(...)` so both the FastAPI app and the FastMCP sub-app run their startup/shutdown logic. ([FastMCP][5])

```python id="1u3bgs"
from contextlib import asynccontextmanager
from fastapi import FastAPI
from fastmcp import FastMCP
from fastmcp.utilities.lifespan import combine_lifespans

@asynccontextmanager
async def app_lifespan(app):
    yield

mcp = FastMCP("Tools")
mcp_app = mcp.http_app()

app = FastAPI(lifespan=combine_lifespans(app_lifespan, mcp_app.lifespan))
app.mount("/mcp", mcp_app)
```

### 19.8 Mounted OAuth-protected servers: `.well-known`, `base_url`, and `mcp_path`

This is the highest-value deployment subtlety in the HTTP docs. When an OAuth-protected FastMCP server is mounted under a path prefix, operational routes and discovery routes obey different rules. The deployment guide says OAuth operational routes move with the mount, but discovery routes must remain at the domain root for RFC compliance. Specifically, operational routes include `/authorize`, `/token`, `/auth/callback`, and `/mcp`; discovery routes include `/.well-known/oauth-authorization-server` and `/.well-known/oauth-protected-resource/*`. ([FastMCP][3])

The guide states the two most common mistakes explicitly:

1. forgetting to mount `.well-known` routes at the root level when the server is mounted under a prefix;
2. including the mount prefix in both `base_url` and `mcp_path`, which yields doubled paths like `/api/api/mcp`. ([FastMCP][3])

The parameter contract is:

* `base_url` includes the external mount prefix, for example `http://localhost:8000/api`
* `mcp_path` is only the internal MCP route, for example `/mcp`
* `issuer_url` is optional and defaults to `base_url`
* key invariant: **`base_url + mcp_path = actual externally accessible MCP URL`** ([FastMCP][3])

That means:

* correct: `base_url="http://localhost:8000/api"` and `mcp_path="/mcp"` → final endpoint `/api/mcp`
* wrong: `base_url="http://localhost:8000/api"` and `mcp_path="/api/mcp"` → final endpoint `/api/api/mcp` ([FastMCP][3])

The deployment guide’s canonical mounted-OAuth pattern is:

```python id="5hvyne"
from fastmcp import FastMCP
from fastmcp.server.auth.providers.github import GitHubProvider
from starlette.applications import Starlette
from starlette.routing import Mount

ROOT_URL = "http://localhost:8000"
MOUNT_PREFIX = "/api"
MCP_PATH = "/mcp"

auth = GitHubProvider(
    client_id="your-client-id",
    client_secret="your-client-secret",
    base_url=f"{ROOT_URL}{MOUNT_PREFIX}",
)

mcp = FastMCP("Protected Server", auth=auth)
mcp_app = mcp.http_app(path=MCP_PATH)

well_known_routes = auth.get_well_known_routes(mcp_path=MCP_PATH)

app = Starlette(
    routes=[
        *well_known_routes,                 # root-level discovery
        Mount(MOUNT_PREFIX, app=mcp_app),  # prefixed operational routes
    ],
    lifespan=mcp_app.lifespan,
)
```

The documented resulting URL set is:

* MCP endpoint: `http://localhost:8000/api/mcp`
* OAuth authorization: `http://localhost:8000/api/authorize`
* OAuth callback: `http://localhost:8000/api/auth/callback`
* Authorization-server metadata: `http://localhost:8000/.well-known/oauth-authorization-server/api`
* Protected-resource metadata: `http://localhost:8000/.well-known/oauth-protected-resource/api/mcp` ([FastMCP][3])

A second subtlety: when `issuer_url` has a path, or when it defaults from a path-bearing `base_url`, FastMCP generates **path-aware** discovery routes, such as `/.well-known/oauth-authorization-server/api`. That behavior is intentional and required for the mounted-path OAuth case. ([FastMCP][3])

### 19.9 Horizontal scaling and `stateless_http`

The HTTP deployment guide says Streamable HTTP is stateful by default: server-side sessions are maintained in memory, enabling MCP features such as elicitation and sampling. That is fine for single-instance deployments but problematic for horizontal scaling because requests from one client may land on different instances. The same guide further says sticky sessions are not reliable for many MCP clients because clients such as Cursor and Claude Code use `fetch()` and do not properly forward `Set-Cookie` headers. ([FastMCP][3])

The documented mitigation is `stateless_http=True`, either through `http_app()`, through `run(transport="http", stateless_http=True)`, or through `FASTMCP_STATELESS_HTTP=true`. In stateless mode, each request creates a fresh transport context, eliminating the need for session affinity. If you plan to run multiple Uvicorn workers or multiple hosts behind a load balancer, treat `stateless_http=True` as the default unless you explicitly require stateful MCP session features. ([FastMCP][3])

```python id="9ikd0y"
# ASGI
app = mcp.http_app(stateless_http=True)

# direct runtime
if __name__ == "__main__":
    mcp.run(transport="http", stateless_http=True)
```

### 19.10 ASGI-only deployment knobs worth knowing

`http_app()` and `run_http_async()` expose a few serve-time knobs that matter for real deployments:

* `middleware=` for ASGI middleware injection
* `json_response=` for response formatting policy
* `event_store=` and `retry_interval=` for resumable long-running event streams
* `stateless_http=` for horizontally scalable deployments
* `path=` for endpoint routing control ([FastMCP][1])

The HTTP deployment guide also notes that for long-running operations with stream reconnection, `http_app(event_store=..., retry_interval=...)` can be backed by in-memory or custom storage, and recommends Redis-backed storage for distributed deployments. That makes `http_app()` the correct primitive when deployment shape, not just functionality, is a design variable. ([FastMCP][3])

### 19.11 Production advisories

**Use STDIO by default for local/editor/desktop/server-subprocess cases.** The docs explicitly position STDIO as perfect for local development, Claude Desktop, command-line tools, and single-user applications. ([FastMCP][2])

**Use `transport="http"` for all new network deployments.** The running guide explicitly recommends HTTP/Streamable HTTP for new network deployments and says SSE should not be used for new projects. ([FastMCP][2])

**Use direct `run(transport="http")` for simple standalone services; use `http_app()` for production infrastructure.** The HTTP deployment guide makes that exact tradeoff: direct runtime for minimal configuration, ASGI for workers, middleware, existing app integration, and deployment-pipeline compatibility. ([FastMCP][3])

**Always wire lifespan when mounting.** Both the Starlette/FastAPI deployment docs and the lifespan docs are explicit that nested lifespans are not recognized automatically; without forwarding `mcp_app.lifespan` (or combining lifespans), FastMCP’s session manager will not initialize correctly. ([FastMCP][3])

**For mounted OAuth-protected servers, root-level `.well-known` routes are mandatory.** FastMCP cannot do this automatically when your protected MCP app is mounted under a prefix; you must retrieve the routes from the auth provider and mount them at the root. ([FastMCP][3])

**Do not duplicate the mount prefix in both `base_url` and `mcp_path`.** The docs call this out as a common mistake and give the exact invariant: `base_url + mcp_path` must equal the externally accessible MCP URL. ([FastMCP][3])

**For multi-worker or load-balanced deployments, default to `stateless_http=True`.** The deployment guide documents why sticky sessions are unreliable with common MCP clients and gives `stateless_http` as the intended solution. ([FastMCP][3])

**If you reverse-proxy Streamable HTTP behind nginx, disable buffering.** The deployment guide states that Streamable HTTP uses Server-Sent Events for streaming responses and gives an nginx example with `proxy_buffering off`, `proxy_cache off`, and extended read/send timeouts to avoid breaking event streams. ([FastMCP][3])


[1]: https://gofastmcp.com/python-sdk/fastmcp-server-mixins-transport "transport - FastMCP"
[2]: https://gofastmcp.com/deployment/running-server "Running Your Server - FastMCP"
[3]: https://gofastmcp.com/deployment/http "HTTP Deployment - FastMCP"
[4]: https://gofastmcp.com/integrations/fastapi "FastAPI  FastMCP - FastMCP"
[5]: https://gofastmcp.com/servers/lifespan "Lifespans - FastMCP"

# FastMCP Advanced — 20) HTTP hardening, reverse proxies, scaling, and event delivery

## 20.0 HTTP deployment is a systems problem

`mcp.run(transport="http")` is enough to make a network-reachable server, but production correctness depends on much more than opening a port:

```text
external URL / TLS
  -> load balancer / reverse proxy
  -> Host / Origin policy
  -> OAuth discovery + callback URLs
  -> FastMCP ASGI app
  -> stateful or stateless HTTP behavior
  -> session/event/state stores
  -> workers / replicas
  -> backend services
```

The deployment must preserve MCP streaming semantics, auth metadata routes, request identity, lifecycle, and any state expectations. ([HTTP Deployment][1])

---

## 20.1 Direct HTTP vs ASGI export

Direct:

```python
if __name__ == "__main__":
    mcp.run(
        transport="http",
        host="0.0.0.0",
        port=8000,
    )
```

ASGI export:

```python
app = mcp.http_app(path="/mcp")
```

Prefer direct HTTP for simple standalone deployments. Prefer ASGI export when you need:

* multiple workers/process manager control;
* custom ASGI middleware;
* FastAPI/Starlette integration;
* shared web routes;
* external server configuration;
* advanced reverse-proxy deployment. ([HTTP Deployment][1])

---

## 20.2 Path composition invariant

Final MCP URL is composed from the outer mount prefix and the internal FastMCP path.

Example:

```python
mcp_app = mcp.http_app(path="/mcp")
app.mount("/analytics", mcp_app)
```

Final path:

```text
/analytics/mcp
```

Alternative:

```python
mcp_app = mcp.http_app(path="/")
app.mount("/mcp", mcp_app)
```

Final path:

```text
/mcp
```

Write the final external endpoint explicitly in deployment configuration/tests; do not infer it informally from one code line.

---

## 20.3 Lifespan forwarding

For mounted apps:

```python
mcp_app = mcp.http_app(path="/mcp")
app = FastAPI(lifespan=mcp_app.lifespan)
app.mount("/analytics", mcp_app)
```

If the outer app already has lifespan behavior, combine lifespans. Without FastMCP's lifecycle, session management and other startup resources can fail. ([FastAPI Integration][2])

---

## 20.4 Stateful HTTP in FastMCP 3

FastMCP 3's Streamable HTTP can maintain server-side MCP session state. This supports 3.x connection/session features but creates a routing problem when multiple workers/replicas are used.

```text
client session -> worker A state
next request -> worker B
              -> state unavailable unless shared/routed
```

Do not add multiple workers to a previously single-process stateful deployment without testing this boundary.

---

## 20.5 `stateless_http=True`

When session-affinity features are not required, use stateless HTTP to make ordinary load balancing practical:

```python
app = mcp.http_app(stateless_http=True)
```

or:

```python
mcp.run(
    transport="http",
    stateless_http=True,
)
```

FastMCP's HTTP guide recommends this for horizontally scaled deployments where MCP session affinity cannot be relied on. ([HTTP Deployment][1])

---

## 20.6 Sticky sessions are not a universal solution

Do not assume cookie-based sticky sessions work with every MCP host. Some hosts use fetch/client behavior that does not preserve proxy cookies as a browser would.

If stateful protocol sessions are mandatory:

* verify the actual MCP session header behavior;
* use shared stores where supported;
* route based on protocol/session identifiers if infrastructure allows;
* test the specific clients you support.

If stateful features are unnecessary, stateless HTTP is simpler.

---

## 20.7 Reverse proxy streaming configuration

Streamable HTTP uses streaming responses. Reverse proxies must not buffer the response until completion.

Typical nginx concepts:

```nginx
proxy_http_version 1.1;
proxy_buffering off;
proxy_cache off;
proxy_read_timeout 3600s;
proxy_send_timeout 3600s;
```

Exact TLS/header/timeouts depend on your infrastructure. The key invariant is: **do not transform a streaming MCP response into a buffered request/response blob**. ([HTTP Deployment][1])

---

## 20.8 Proxy timeout layering

There may be several timeout layers:

```text
client request timeout
reverse proxy timeout
ASGI server timeout
FastMCP tool timeout
backend API timeout
load balancer idle timeout
```

The smallest relevant timeout wins. For long-running work, use background tasks rather than setting every network timeout to hours.

---

## 20.9 Host/Origin protection

FastMCP 3.4.3 introduced Host/Origin validation for DNS-rebinding protection; 3.4.4 restored compatibility by making strict protection opt-in for deployments whose public host/origin is known.

A hardened deployment can explicitly configure concepts such as:

```text
host_origin_protection=True
allowed_hosts=[...]
allowed_origins=[...]
```

Use the exact 3.4.7 API/configuration location documented by your runtime. Test the public Host header as seen **after** any reverse proxy. ([FastMCP Updates][3])

---

## 20.10 DNS rebinding threat model

Binding to localhost does not automatically make an HTTP MCP endpoint safe from browser-origin attacks.

Threat shape:

```text
malicious web origin
   -> DNS response / rebinding trick
   -> browser sends request toward localhost service
   -> weak Host/Origin validation accepts it
```

Mitigations include Host/Origin validation, authentication, browser-origin policy, and not exposing unnecessary unauthenticated local network services.

---

## 20.11 OAuth mounted-path invariant

For an authenticated server mounted under `/api`:

```text
base_url = https://example.com/api
mcp_path = /mcp
final endpoint = https://example.com/api/mcp
```

Do not set:

```text
base_url = https://example.com/api
mcp_path = /api/mcp
```

which double-applies the prefix. FastMCP's HTTP docs explicitly call out the invariant:

```text
base_url + mcp_path = externally reachable MCP endpoint
```

([HTTP Deployment][1])

---

## 20.12 `.well-known` discovery routes

OAuth discovery metadata has standards-defined root-level routing semantics. When mounting an authenticated FastMCP application under a prefix, the operational routes may move with the application but `.well-known` discovery routes may need explicit root-level mounting.

Conceptual:

```text
/.well-known/...                 -> discovery metadata
/api/authorize                   -> mounted operational route
/api/token                       -> mounted operational route
/api/mcp                         -> mounted MCP endpoint
```

Do not hide `.well-known` metadata under a nested ASGI mount and assume OAuth clients will discover it. ([HTTP Deployment][1])

---

## 20.13 External URL awareness behind proxies

OAuth redirects and metadata must describe the **external** scheme/host/path, not the internal Uvicorn address.

Internal:

```text
http://10.0.2.17:8000/mcp
```

External:

```text
https://mcp.example.com/api/mcp
```

Configure trusted forwarded headers/proxy behavior carefully so the application does not trust spoofed `X-Forwarded-*` values from arbitrary clients.

---

## 20.14 Event stores

FastMCP's ASGI/runtime APIs expose an event-store concept for resumable streaming/event delivery. Use an external/shared event store when continuity must survive worker changes or reconnects.

Design questions:

* event ID uniqueness;
* retention/TTL;
* replay ordering;
* consumer/session isolation;
* backend failure behavior;
* cross-replica consistency;
* cleanup and maximum storage.

In-memory event storage is appropriate only when process-local continuity is acceptable. ([HTTP Deployment][1])

---

## 20.15 Retry interval and reconnect behavior

If an event-store/retry mechanism is enabled, treat retry intervals as client/network behavior, not task polling. A reconnecting stream must resume without duplicating side effects.

Tool execution should be idempotent or separately deduplicated where a network retry could otherwise cause duplicate work.

---

## 20.16 Multiple Uvicorn workers

Before enabling:

```bash
uvicorn app:app --workers 4
```

verify:

```text
[ ] stateless_http or compatible shared session strategy
[ ] shared session state store if session state is used
[ ] shared task backend if workers execute/observe tasks
[ ] external/shared event store if resume requires it
[ ] no correctness-critical module-global mutable state
[ ] every worker can access identical provider/config/catalog sources
[ ] OAuth redirect/base URL not worker-local
```

Process count is an architecture change, not merely a throughput tuning flag.

---

## 20.17 Load balancer health checks

Use a simple HTTP health route for process/load-balancer checks, but also implement deeper readiness checks for systems that rely on upstream MCP providers/databases.

Examples:

```text
/livez   -> process/event loop alive
/readyz  -> required dependencies initialized
```

A ProxyProvider gateway can be "alive" while all upstream MCP servers are unreachable; readiness should reflect the service contract you promise.

---

## 20.18 Custom routes

FastMCP can add lightweight HTTP custom routes such as health checks:

```python
from starlette.requests import Request
from starlette.responses import JSONResponse


@mcp.custom_route("/health", methods=["GET"])
async def health(request: Request):
    return JSONResponse({"status": "ok"})
```

Use an actual FastAPI/Starlette parent app for complex REST/web functionality; custom routes are not meant to turn FastMCP into a full general web framework. ([Server][4])

---

## 20.19 TLS termination

Production HTTP MCP endpoints should generally use HTTPS. TLS can terminate at:

* cloud load balancer;
* ingress/reverse proxy;
* ASGI server directly.

If TLS terminates upstream, ensure OAuth metadata/redirect URLs still use `https://` externally and that trusted-proxy configuration accurately reconstructs the public request.

---

## 20.20 SSRF-safe auth metadata/JWKS networking

FastMCP 3.4.x hardened OAuth metadata/JWKS fetching and added support for trusted corporate proxies. Do not bypass these protections with a custom generic network fetcher that follows arbitrary redirects or directly accesses private IP ranges.

If enterprise egress requires a proxy, configure it explicitly and fail closed when the security policy requires it. ([FastMCP Updates][3])

---

## 20.21 Container deployment pattern

Conceptual Dockerfile:

```dockerfile
FROM python:3.12-slim
WORKDIR /app

COPY pyproject.toml uv.lock ./
RUN pip install uv && uv sync --frozen --no-dev

COPY src ./src
COPY fastmcp.json ./fastmcp.json

EXPOSE 8000
CMD ["uv", "run", "fastmcp", "run", "fastmcp.json", "--transport", "http", "--host", "0.0.0.0", "--port", "8000"]
```

Treat the exact CLI options as versioned; an ASGI module plus `uvicorn` may be preferable when using multiple workers or custom middleware.

---

## 20.22 Serverless suitability

Stateless HTTP helps serverless/container-autoscaling deployments, but evaluate:

* cold-start cost of imports/providers;
* lifespan initialization latency;
* external state/task backends;
* maximum request duration;
* streaming support of the platform;
* OAuth callback stability;
* filesystem ephemerality;
* connection/reconnect behavior.

A serverless platform that buffers streaming responses may be incompatible even if normal JSON HTTP works.

---

## 20.23 HTTP hardening matrix

| Concern | Single local HTTP | Internal service | Public multi-replica |
|---|---|---|---|
| TLS | optional dev | usually yes | yes |
| auth | depends | usually | yes |
| Host/Origin policy | recommended | explicit | explicit |
| stateless HTTP | optional | evaluate | usually prefer unless session features required |
| shared session store | no | if stateful multi-worker | if stateful |
| event store | optional | if resumability needed | shared if resumability needed |
| reverse-proxy buffering | n/a | disable for streaming | disable |
| `.well-known` root routes | if OAuth | if OAuth | if OAuth |
| SSRF/trusted proxy | auth-provider dependent | likely | required review |

---

## 20.24 Failure-mode diagnostics

| Symptom | Check |
|---|---|
| works locally, 404 behind ingress | mount prefix/internal path |
| OAuth discovery fails | `.well-known` routing / external base URL |
| redirect returns wrong host/scheme | forwarded headers/public URL config |
| intermittent session failures with workers | stateful HTTP routed across workers |
| stream arrives only at end | proxy buffering |
| connections drop around fixed interval | LB/proxy/read timeout |
| localhost requests rejected after 3.4.3 | Host/Origin protection config; update to compatible patch |
| secure metadata fetch cannot reach IdP | trusted proxy/CA/SSRF policy |
| app mounts but session manager fails | lifespan not forwarded |

---

## 20.25 Anti-pattern inventory

* adding workers without evaluating stateful session behavior;
* assuming sticky cookies work with every MCP client;
* enabling proxy buffering for Streamable HTTP;
* using internal Uvicorn URL as OAuth public `base_url`;
* nesting `.well-known` routes under an arbitrary mount prefix;
* duplicating mount prefix in both `base_url` and `mcp_path`;
* trusting forwarded headers from any source;
* disabling Host/Origin protections because one proxy was misconfigured instead of fixing the public host model;
* custom OAuth metadata fetches that bypass SSRF defenses;
* health checks that ignore required upstream MCP providers;
* putting long-running work in foreground requests and simply increasing all timeouts.

---

## 20.26 HTTP deployment checklist

```text
[ ] Record exact external MCP URL.
[ ] Verify mount prefix + internal MCP path composition.
[ ] Forward/combine FastMCP lifespan.
[ ] Decide stateful vs stateless HTTP explicitly.
[ ] If multi-worker, externalize required state/task/event stores.
[ ] Disable reverse-proxy buffering for streaming.
[ ] Set coherent timeout budgets across every network layer.
[ ] Configure public base_url / OAuth callback metadata.
[ ] Mount required .well-known routes correctly.
[ ] Configure Host/Origin policy for known public endpoints.
[ ] Preserve SSRF-safe auth metadata/JWKS fetching.
[ ] Test through the real load balancer/reverse proxy, not only localhost.
```

[1]: https://gofastmcp.com/deployment/http "HTTP Deployment"
[2]: https://gofastmcp.com/integrations/fastapi "FastAPI Integration"
[3]: https://gofastmcp.com/updates "FastMCP Updates"
[4]: https://gofastmcp.com/servers/server "FastMCP Server"

# FastMCP Advanced — 21) Programmatic client fundamentals
### Programmatic client fundamentals

### 21.0 Role and operating model

`fastmcp.Client` is a **deterministic, explicit, programmatic** MCP client. The docs position it as a typed building block for controlled interactions rather than an autonomous agent runtime: you call explicit methods, manage connection scope deliberately, and receive typed result objects instead of conversational side effects. Internally, the `Client` owns MCP protocol logic while a `Transport` owns connection establishment and lifecycle. ([FastMCP][1])

The client is also designed for **reentrant** usage. The SDK says it supports multiple concurrent or nested `async with client:` blocks via reference counting and background session management, specifically to avoid race conditions exposed by MCP SDK behavior such as automatic `list_tools()` calls during tool execution. That means “use `async with` everywhere” and “reuse one configured client safely in nested code paths” are compatible design choices. ([FastMCP][2])

### 21.1 Constructor surface and transport inference

The `Client` constructor accepts a **transport/source specification** plus optional handlers and timeouts. The SDK documents the accepted source categories as: a `ClientTransport` instance, an in-process `FastMCP` server, a URL / `str`, a `Path`, an `MCPConfig`, or a configuration `dict`; optional constructor concerns include roots, sampling/log/message/progress handlers, request timeout, and initialization timeout. ([FastMCP][2])

FastMCP automatically infers the concrete transport from what you pass. The transport-inference SDK is explicit:

* `FastMCP` instance → in-memory transport
* file path (`Path` or `str`) → Python STDIO transport for `.py`, Node STDIO transport for `.js`
* URL (`AnyUrl` or `str`) → `StreamableHttpTransport` by default, or `SSETransport` for `/sse` endpoints
* `MCPConfig` / config `dict` → `MCPConfigTransport`, potentially creating a composite multi-server client. ([FastMCP][3])

Representative forms:

```python id="7u4lbo"
from fastmcp import Client, FastMCP

server = FastMCP("TestServer")

client_a = Client(server)                       # in-memory
client_b = Client("https://example.com/mcp")   # HTTP
client_c = Client("my_server.py")              # inferred STDIO
client_d = Client({
    "mcpServers": {
        "weather": {"url": "https://weather.example.com/mcp"},
        "assistant": {"command": "python", "args": ["./assistant.py"]},
    }
})
```

That source-inference behavior is the core reason the client feels compact at call sites while still supporting very different deployment topologies. ([FastMCP][1])

### 21.2 `async with` lifecycle, connection establishment, and initialization

The high-level client docs are explicit: all client operations should run inside `async with client:` so connection setup and teardown happen correctly. Entering the context establishes the transport connection and performs the MCP initialization handshake automatically unless `auto_initialize=False` was configured. The handshake exchanges capabilities, server metadata, protocol version, and optional server instructions. ([FastMCP][1])

```python id="lymwxz"
import asyncio
from fastmcp import Client, FastMCP

mcp = FastMCP(name="MyServer", instructions="Use the greet tool to say hello!")

@mcp.tool
def greet(name: str) -> str:
    return f"Hello, {name}!"

async def main():
    async with Client(mcp) as client:
        await client.ping()
        print(client.initialize_result.serverInfo.name)
        print(client.initialize_result.instructions)

asyncio.run(main())
```

The SDK also documents manual initialization for advanced cases. `initialize(timeout=...)` is idempotent, returns the cached first result on repeated calls, and is only necessary when `auto_initialize=False` was set during client construction. This is the right pattern when you need to connect first, inspect connection state, or customize initialization timing separately from ordinary operations. ([FastMCP][1])

```python id="aa84mu"
from fastmcp import Client

client = Client("my_server.py", auto_initialize=False)

async with client:
    assert client.is_connected()
    assert client.initialize_result is None

    init = await client.initialize(timeout=10.0)
    print(init.serverInfo.name)

    tools = await client.list_tools()
```

### 21.3 Transport choice and deployment implications

#### In-memory transport

Passing a `FastMCP` instance creates an in-memory transport. The docs describe this as direct same-process interaction with no subprocess or network overhead, sharing the same memory space and environment variables as the client process. This is the best transport for testing, local evaluation harnesses, and deterministic integration tests. ([FastMCP][1])

#### STDIO transport

Passing a local script path infers STDIO transport, but the transport docs warn that this is the **limited** convenience path. For full control, instantiate `StdioTransport` explicitly with `command`, `args`, `env`, and `cwd`. The key operational rule is explicit in the docs: STDIO servers **do not inherit** your shell environment by default, so required environment variables must be passed intentionally. ([FastMCP][1])

```python id="eh9bej"
from fastmcp import Client
from fastmcp.client.transports import StdioTransport

transport = StdioTransport(
    command="python",
    args=["my_server.py", "--verbose"],
    env={"API_KEY": "secret", "LOG_LEVEL": "DEBUG"},
    cwd="/path/to/server",
)

client = Client(transport)
```

STDIO also supports subprocess session persistence by default (`keep_alive=True`), so repeated `async with client:` blocks can reuse the same subprocess. Disable that explicitly when you need per-connection isolation. ([FastMCP][4])

#### HTTP transport

HTTP is the recommended production transport. The docs describe it as the right fit when the server runs independently and manages its own lifecycle. The explicit transport class is `StreamableHttpTransport`, and you can configure headers, auth, and SSL verification through either the transport or the `Client(...)` constructor. ([FastMCP][4])

```python id="qlzodt"
from fastmcp import Client
from fastmcp.client.transports import StreamableHttpTransport

transport = StreamableHttpTransport(
    url="https://api.example.com/mcp",
    headers={"Authorization": "Bearer your-token-here"},
)

client = Client(transport)
```

The transport docs also show constructor-level TLS controls such as `verify=False`, custom CA bundle paths, or custom `ssl.SSLContext` objects. Those are deployment-time controls, not protocol concerns, and belong in the client configuration when talking to internal or self-signed HTTPS endpoints. ([FastMCP][4])

#### SSE transport

SSE remains supported for backward compatibility, but the current client transport docs explicitly recommend Streamable HTTP for new deployments unless you have specific infrastructure reasons to stay on SSE. That makes SSE a compatibility transport, not a new-default design target. ([FastMCP][4])

#### MCPConfig / multi-server composition

If you pass an `MCPConfig` or config `dict`, `Client(...)` can connect to one or more servers. The config-transport SDK says a single-server config becomes a direct transport; a multi-server config becomes a composite client by mounting the servers behind one interface, with tool names prefixed as `{server_name}_{tool_name}` and resource URIs prefixed as `protocol://{server_name}/path`. ([FastMCP][3])

### 21.4 Connection-state helpers and session reuse

The SDK exposes a few state/lifecycle helpers that matter in advanced client code:

* `is_connected()` tells you whether the transport/session is currently active
* `initialize_result` holds the cached MCP initialization response
* `session` returns the active low-level session and raises if disconnected
* `new()` clones the client configuration but gives you fresh disconnected session state. ([FastMCP][2])

`new()` is the correct primitive when you want the same transport/auth/handler configuration but need an independent session—for example, parallel evaluations that should not share roots, state, or handshake context. ([FastMCP][2])

### 21.5 `ping()` — lowest-friction liveness check

`ping()` is the simplest connectivity primitive. The SDK defines it as `ping(self) -> bool` and the high-level client docs show it as the first operation typically issued after entering the client context. Use it for liveness checks, quick smoke tests, or session validation before more expensive operations. ([FastMCP][2])

```python id="4u1rwi"
async with client:
    ok = await client.ping()
    assert ok
```

### 21.6 Listing operations: `list_tools()`, `list_resources()`, `list_prompts()`

The client exposes three primary listing methods:

* `list_tools(...) -> list[mcp.types.Tool]`
* `list_resources(...) -> list[mcp.types.Resource]`
* `list_prompts(...) -> list[mcp.types.Prompt]`

The SDK mixins state that all three automatically paginate through all result pages by default, up to `max_pages=250`, and raise if the page limit is exceeded before pagination completes. For manual pagination control or access to raw protocol metadata, use the corresponding `*_mcp()` methods instead. ([FastMCP][5])

```python id="0qbgdr"
async with client:
    tools = await client.list_tools()
    resources = await client.list_resources()
    prompts = await client.list_prompts()
```

This distinction—high-level convenience list vs raw protocol list—is fundamental:

* `list_tools_mcp()` returns `mcp.types.ListToolsResult`
* `list_resources_mcp()` returns `mcp.types.ListResourcesResult`
* `list_prompts_mcp()` returns `mcp.types.ListPromptsResult`

Use the raw forms when you need cursors or complete protocol payloads; use the convenience forms when you want the fully collected catalog immediately. ([FastMCP][6])

One subtlety on the resource side: concrete resources and resource templates are listed separately. `list_resources()` returns `Resource` objects; `list_resource_templates()` returns `ResourceTemplate` objects. `read_resource(...)` can resolve either a concrete resource URI or a filled-in template URI, but discovery is split across the two listing methods. ([FastMCP][5])

### 21.7 `call_tool(...)` — executable operation with typed result hydration

`call_tool(...)` is the client’s imperative operation entrypoint. The tools mixin SDK documents the current effective surface as: tool `name`, optional `arguments`, optional `version`, optional `timeout`, optional `progress_handler`, optional `raise_on_error`, optional request `meta`, and optional task controls (`task`, `task_id`, `ttl`). The raw-protocol twin `call_tool_mcp(...)` returns the underlying `mcp.types.CallToolResult` and does **not** raise on tool-level failure. ([FastMCP][6])

Basic usage:

```python id="fi1v9g"
async with client:
    result = await client.call_tool("add", {"a": 5, "b": 3})
```

The high-level client docs define the return as `CallToolResult` and spell out its core properties:

* `.data`: FastMCP-only hydrated Python value reconstructed from the server’s output schema
* `.content`: standard MCP content blocks (`TextContent`, `ImageContent`, `AudioContent`, etc.)
* `.structured_content`: raw structured JSON payload from the server
* `.is_error`: boolean tool-failure marker. ([FastMCP][7])

This yields the single most important result-model distinction in the whole client API:

* use `.data` when you want **deserialized Python objects** with complex types restored;
* use `.structured_content` when you want the **raw JSON structure** exactly as sent;
* use `.content` when you want the **protocol-standard content blocks** suitable for text/media handling. ([FastMCP][7])

Representative access pattern:

```python id="e8jcmz"
async with client:
    result = await client.call_tool("get_weather", {"city": "London"})

    python_value = result.data
    raw_json = result.structured_content
    first_block = result.content[0]
```

The docs also state an important fallback rule: if the tool has no output schema, or deserialization fails, `.data` will be `None`; in that case fall back to `.content`. Primitive tool outputs are automatically wrapped on the server as `{"result": value}` and unwrapped by FastMCP on the client so `.data` is the original primitive. ([FastMCP][7])

Error behavior is also explicit. `call_tool()` raises `ToolError` by default when the server reports tool failure; `raise_on_error=False` disables that and lets you inspect `.is_error` and `.content` manually. If you need the raw MCP result object with `isError` and no client-side exception behavior, use `call_tool_mcp(...)` instead. ([FastMCP][7])

```python id="i0okek"
from fastmcp.exceptions import ToolError

async with client:
    try:
        result = await client.call_tool("potentially_failing_tool", {"param": "value"})
    except ToolError:
        ...

    raw = await client.call_tool_mcp("potentially_failing_tool", {"param": "value"})
    if raw.isError:
        ...
```

### 21.8 `read_resource(...)` — URI-addressed content retrieval

`read_resource(...)` is the resource-side read primitive. The resources mixin SDK documents the effective surface as: `uri`, optional `version`, optional request `meta`, and optional task controls (`task`, `task_id`, `ttl`). The raw twin `read_resource_mcp(...)` returns `mcp.types.ReadResourceResult`; the convenience form returns either a content list or a `ResourceTask` when background execution is requested. ([FastMCP][5])

Basic read:

```python id="6vh7m7"
async with client:
    content = await client.read_resource("file:///path/to/README.md")
```

The high-level resource docs say the return is a list of `TextResourceContents | BlobResourceContents`. Text items expose `.text` and `.mimeType`; binary items expose `.blob` and `.mimeType`. Resource templates are read through the same method by supplying a concrete filled-in URI like `weather://london/current`. ([FastMCP][8])

```python id="jlwmfs"
async with client:
    content = await client.read_resource("resource://config/settings.json")
    if hasattr(content[0], "text"):
        print(content[0].text)
        print(content[0].mimeType)

    binary = await client.read_resource("resource://images/logo.png")
    if hasattr(binary[0], "blob"):
        print(len(binary[0].blob))
        print(binary[0].mimeType)
```

This is the key result-model difference from tools: `read_resource(...)` does **not** return a `.data`-bearing hydration object. Its primary abstraction is the content-item list itself, with `.text`/`.blob` on each element. If you want a typed Python object from resource data, that is your responsibility after reading the textual or binary content. ([FastMCP][8])

### 21.9 `get_prompt(...)` — rendered message retrieval

`get_prompt(...)` retrieves a rendered prompt result from the server. The prompts mixin SDK documents the effective surface as: prompt `name`, optional `arguments`, optional `version`, optional request `meta`, and optional task controls (`task`, `task_id`, `ttl`). The raw and convenience forms are `get_prompt_mcp(...)` and `get_prompt(...)`; both return a `GetPromptResult` when not using background execution. ([FastMCP][9])

Basic usage:

```python id="hlx5p3"
async with client:
    result = await client.get_prompt("welcome_message")
    for message in result.messages:
        print(message.role, message.content)
```

The client prompt docs explicitly say `get_prompt()` returns `mcp.types.GetPromptResult`, and that the meaningful payload is `result.messages`. Each rendered message has a `role` and `content`; the docs show content access either directly or via `.content.text` when the content object is text-shaped. ([FastMCP][10])

This is the fundamental contrast with tools/resources:

* tool call → one `CallToolResult` with `.data`, `.content`, `.structured_content`
* resource read → list of resource-content items with `.text` / `.blob`
* prompt render → `GetPromptResult` whose main payload is `.messages`. ([FastMCP][7])

Prompt arguments also have a special serialization rule. The current prompt client docs say FastMCP automatically serializes complex prompt arguments to JSON strings as required by MCP, so you can pass dataclasses, dicts, and lists directly from Python and the client will encode them appropriately before sending the request. ([FastMCP][10])

```python id="7nriay"
from dataclasses import dataclass

@dataclass
class UserData:
    name: str
    age: int

async with client:
    result = await client.get_prompt("analyze_user", {
        "user": UserData(name="Alice", age=30),
        "preferences": {"theme": "dark"},
        "scores": [85, 92, 78],
    })
```

### 21.10 `.data` vs `.content` vs rendered prompt messages

This distinction is worth stating as a standalone contract:

For **tool calls**, `.data` is the FastMCP-specific typed/hydrated Python value reconstructed from output schema, `.structured_content` is the raw structured JSON, and `.content` is the protocol-standard content-block list. When no output schema exists or hydration fails, `.data` is `None`, but `.content` still exists. ([FastMCP][7])

For **resource reads**, there is no `.data` abstraction. The result is the content list itself, and each item is either text-like (`.text`) or binary-like (`.blob`), with `.mimeType` to guide interpretation. ([FastMCP][8])

For **prompt renders**, the meaningful result is the rendered conversation/message sequence in `GetPromptResult.messages`; each message carries `role` and `content`. Prompt rendering is therefore message-first, not data-first and not content-block-first in the same way tools are. ([FastMCP][10])

A practical rule follows: if you want typed Python data back, prefer tool calls with output schemas. If you want passive text/blob payloads addressed by URI, use resources. If you want reusable rendered conversational scaffolding, use prompts. The client API mirrors those three server component roles directly. ([FastMCP][1])

### 21.11 Raw protocol access vs convenience methods

Each major operation has a raw-protocol twin:

* `call_tool_mcp(...) -> mcp.types.CallToolResult`
* `read_resource_mcp(...) -> mcp.types.ReadResourceResult`
* `get_prompt_mcp(...) -> mcp.types.GetPromptResult`
* plus `list_*_mcp()` methods for full paginated list results. ([FastMCP][6])

Use the convenience methods when you want automatic pagination, automatic prompt-argument serialization, typed tool-result hydration, and default exception behavior. Use the raw MCP methods when you need the wire-level response object, cursor metadata, raw `isError` handling, or you are building a higher-level client abstraction that should control deserialization and error policy itself. ([FastMCP][7])

### 21.12 Deployment and authoring advisories

Use **in-memory** transport for tests and same-process evaluation harnesses; use **STDIO** when the client should launch a local server subprocess; use **HTTP / Streamable HTTP** for production services; use **SSE** only for backward compatibility with older deployments or clients. The transport docs are explicit about each of those recommendations. ([FastMCP][4])

Do not rely on file-path inference for anything beyond the simplest local usage. The docs explicitly say it is a convenience path with limited configuration, whereas explicit `StdioTransport(...)` lets you set command, args, environment, and working directory. That is especially important because STDIO servers do **not** inherit your shell environment by default. ([FastMCP][4])

Use `async with client:` as the default lifecycle shape everywhere, even though the client supports reentrant contexts. That gives you correct connection management, automatic initialization, and a predictable cleanup boundary; drop to manual `initialize()` only when you explicitly need deferred handshake control. ([FastMCP][1])

If you need isolated sessions with the same configuration, use `client.new()` rather than mutating one shared client instance unpredictably. If you need transport-specific control—headers, auth, SSL verification, env, or keep-alive—instantiate the transport explicitly rather than relying on inference. ([FastMCP][2])


[1]: https://gofastmcp.com/clients/client "The FastMCP Client - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-client-client "client - FastMCP"
[3]: https://gofastmcp.com/python-sdk/fastmcp-client-transports-inference "inference - FastMCP"
[4]: https://gofastmcp.com/clients/transports "Client Transports - FastMCP"
[5]: https://gofastmcp.com/python-sdk/fastmcp-client-mixins-resources "resources - FastMCP"
[6]: https://gofastmcp.com/python-sdk/fastmcp-client-mixins-tools "tools - FastMCP"
[7]: https://gofastmcp.com/clients/tools "Calling Tools - FastMCP"
[8]: https://gofastmcp.com/clients/resources "Reading Resources - FastMCP"
[9]: https://gofastmcp.com/python-sdk/fastmcp-client-mixins-prompts "prompts - FastMCP"
[10]: https://gofastmcp.com/clients/prompts "Getting Prompts - FastMCP"

# FastMCP Advanced — 22) Client transports, handlers, roots, and client-side auth
### Client transports, handlers, roots, and client-side auth

### 22.0 Scope and constructor surface

`Client(...)` is transport-driven. The SDK documents the constructor as accepting a connection source plus client-side runtime hooks: `transport`/source, `roots`, `sampling_handler`, `log_handler`, `message_handler`, `progress_handler`, `timeout`, and `init_timeout`. The source can be a concrete transport instance, a `FastMCP` server, a URL/string, a local path, an `MCPConfig`, or a configuration dict. This means “transport selection” and “interactive capability callbacks” are both first-class constructor concerns, not later bolt-ons. ([FastMCP][1])

At a design level, the client should be configured around three orthogonal axes: **transport** (how to connect), **auth** (how to authenticate over HTTP transports), and **handlers/roots** (how the client responds when the server calls back into it for sampling, elicitation, progress, logging, notifications, or root discovery). Keeping those axes separate avoids one of the most common generated-code mistakes: mixing transport wiring, OAuth state, and user-interaction callbacks in ad hoc request code instead of at client construction time. ([FastMCP][1])

### 22.1 Transport inference vs explicit transport objects

FastMCP can infer transports from what you pass to `Client(...)`: a `FastMCP` instance gives in-memory transport, a file path gives STDIO, a URL gives HTTP or SSE depending on the path, and a config object/dict can produce single-server or composite multi-server transports. The docs repeatedly note that explicit transport instances give full control over configuration, while inference is mainly a convenience layer. ([FastMCP][2])

The practical rule for agent authors is:

* use **explicit transport objects** in production or any nontrivial local setup;
* use **inference** only for quick tests, examples, or one-off scripts. ([FastMCP][3])

---

## 22.2 `StdioTransport` — subprocess-controlled local transport

STDIO transport communicates with servers through subprocess pipes. The client launches and manages the server process, controlling its lifecycle and environment. FastMCP’s docs call this the standard mechanism for local desktop-style MCP use and the right choice when you need a local server process under explicit client control. ([FastMCP][3])

Canonical explicit form:

```python id="ldv6z2"
from fastmcp import Client
from fastmcp.client.transports import StdioTransport

transport = StdioTransport(
    command="python",
    args=["my_server.py", "--verbose"],
    env={"API_KEY": "secret", "LOG_LEVEL": "DEBUG"},
    cwd="/path/to/server",
)

client = Client(transport)
```

This shape is directly documented and should be preferred over path inference whenever environment, arguments, or working directory matter. ([FastMCP][3])

### 22.2.1 Critical environment rule: nothing is inherited automatically

The docs are explicit: STDIO servers run in isolated environments by default and do **not** inherit your shell environment variables. You must pass needed configuration explicitly via `env=...`. FastMCP even documents selective forwarding and `.env` loading as the standard patterns for this. ([FastMCP][3])

Selective forwarding pattern:

```python id="e4j8zl"
import os
from fastmcp import Client
from fastmcp.client.transports import StdioTransport

required_vars = ["API_KEY", "DATABASE_URL", "REDIS_HOST"]
env = {var: os.environ[var] for var in required_vars if var in os.environ}

client = Client(
    StdioTransport(
        command="python",
        args=["server.py"],
        env=env,
    )
)
```

For generated deployment code, this should be treated as a hard rule, not a suggestion. “It worked in my shell” is not evidence that it will work under STDIO client launch. ([FastMCP][3])

### 22.2.2 Path inference is intentionally limited

`Client("my_server.py")` infers STDIO transport, but the docs explicitly label this as limited because it gives you no direct control over environment variables, working directory, or other subprocess settings. Use it only when the server needs no external configuration or when simplicity matters more than reproducibility. ([FastMCP][3])

```python id="t3a8di"
from fastmcp import Client

client = Client("my_server.py")  # Convenient, but limited
```

### 22.2.3 Session persistence and subprocess reuse

`StdioTransport` defaults to `keep_alive=True`, meaning the same subprocess can be reused across multiple `async with client:` blocks. The docs present this as a performance feature; set `keep_alive=False` when you need isolation between client contexts instead of reuse. ([FastMCP][3])

```python id="pi8ruu"
from fastmcp.client.transports import StdioTransport
from fastmcp import Client

client = Client(
    StdioTransport(
        command="python",
        args=["server.py"],
        keep_alive=False,   # fresh subprocess per connection
    )
)
```

Use reuse for local productivity and repeated calls to the same stable subprocess. Disable reuse for tests that require a fresh process boundary, for stateful servers that should not leak state across sessions, or when debugging startup behavior. ([FastMCP][3])

---

## 22.3 `StreamableHttpTransport` — production HTTP transport

FastMCP’s transport docs are explicit: HTTP transport connects to MCP servers running as web services and is the **recommended transport for production deployments**. This is the correct client transport for independently managed remote services, shared team servers, and internet- or intranet-reachable MCP endpoints. ([FastMCP][3])

Canonical explicit form:

```python id="q4tg4w"
from fastmcp import Client
from fastmcp.client.transports import StreamableHttpTransport

transport = StreamableHttpTransport(
    url="https://api.example.com/mcp",
    headers={
        "Authorization": "Bearer your-token-here",
        "X-Custom-Header": "value",
    },
)

client = Client(transport)
```

This is the default recommendation for new production HTTP clients. Use it unless you have a specific reason to stay on SSE. ([FastMCP][3])

### 22.3.1 TLS / certificate verification

The docs say HTTPS verification is enabled by default and the `verify` parameter accepts the same values as `httpx`: `False`, a CA bundle path, or a custom `ssl.SSLContext`. The same parameter exists on both `StreamableHttpTransport` and `SSETransport`. ([FastMCP][3])

```python id="jlwmkl"
import ssl
from fastmcp import Client

client_a = Client("https://dev-server.internal/mcp", verify=False)
client_b = Client("https://corp-server.internal/mcp", verify="/path/to/ca-bundle.pem")

ctx = ssl.create_default_context()
ctx.load_verify_locations("/path/to/internal-ca.pem")
client_c = Client("https://corp-server.internal/mcp", verify=ctx)
```

Use `verify=False` only for local development or controlled internal testing. For enterprise/internal deployments, prefer a custom CA bundle or SSL context rather than disabling verification entirely. That is a security recommendation grounded in the documented verification surface. ([FastMCP][3])

---

## 22.4 `SSETransport` — backward-compatibility transport

FastMCP’s client transport docs state plainly that SSE is maintained for backward compatibility and that **Streamable HTTP should be used for new deployments unless specific infrastructure requirements force SSE**. This is one of the clearest transport recommendations in the docs. ([FastMCP][3])

```python id="1nnv2v"
from fastmcp import Client
from fastmcp.client.transports import SSETransport

transport = SSETransport(
    url="https://api.example.com/sse",
    headers={"Authorization": "Bearer token"},
)

client = Client(transport)
```

Use SSE only when you are integrating with an older server/client ecosystem that already depends on it or when surrounding infrastructure explicitly expects SSE rather than Streamable HTTP. Do not choose it as a fresh default. ([FastMCP][3])

---

## 22.5 In-memory client/server connections

Passing a `FastMCP` server instance to `Client(...)` yields an in-memory transport. The client docs call this ideal for testing and development because it eliminates subprocess and network overhead, and—unlike STDIO—the server shares the same memory space and environment variables as the client code. ([FastMCP][2])

```python id="azjjlwm"
from fastmcp import FastMCP, Client
import os

mcp = FastMCP("TestServer")

@mcp.tool
def greet(name: str) -> str:
    prefix = os.environ.get("GREETING_PREFIX", "Hello")
    return f"{prefix}, {name}!"

client = Client(mcp)
```

Use in-memory transport for tests, benchmarks, local harnesses, and any environment where “same process, no IPC/network” is exactly what you want. Do not mistake it for a production deployment transport; its value is determinism and zero-overhead local integration. ([FastMCP][3])

---

## 22.6 Configuration-based / multi-server transports

FastMCP supports config-driven clients that connect to multiple servers through one `Client(...)`. The transport docs show a configuration dict with `mcpServers`, and the example demonstrates that tool names become namespaced by server (`weather_get_forecast`, `assistant_ask`). This is the correct transport-level composition primitive on the client side. ([FastMCP][3])

```python id="wnq0it"
from fastmcp import Client

config = {
    "mcpServers": {
        "weather": {
            "url": "https://weather.example.com/mcp",
            "transport": "http",
        },
        "assistant": {
            "command": "python",
            "args": ["./assistant.py"],
            "env": {"LOG_LEVEL": "INFO"},
        },
    }
}

client = Client(config)
```

Use this when the caller should see a composed client view but you do not want to build a server-side proxy/orchestrator first. The tradeoff is client-side namespacing complexity instead of server-side composition. ([FastMCP][3])

---

## 22.7 Client-side auth overview: transport scope

FastMCP client auth helpers are only meaningful for **HTTP-based transports**. The bearer-auth docs say bearer auth is only relevant for HTTP transports; the OAuth docs say OAuth auth is only relevant for HTTP-based transports and requires browser interaction; the CIMD docs say CIMD is only relevant for HTTP-based transports and requires server support. STDIO and in-memory transports do not use these HTTP auth mechanisms. ([FastMCP][4])

The practical split is:

* **Bearer auth** for non-interactive tokens you already possess
* **OAuth** for interactive user login / consent flows
* **CIMD** as an OAuth enhancement for verifiable client identity on supporting servers. ([FastMCP][4])

---

## 22.8 `auth="<token>"` and `BearerAuth(...)`

FastMCP’s simplest bearer-auth path is to pass the raw token string to `auth=` on `Client(...)` or on an HTTP transport. The docs are explicit that FastMCP will add the `Bearer` scheme automatically, and you should **not** include the `Bearer ` prefix yourself. This is positioned as the best fit for service accounts, long-lived API keys, CI/CD, or applications where authentication is managed elsewhere. ([FastMCP][4])

```python id="xn1oxr"
from fastmcp import Client

async with Client(
    "https://your-server.fastmcp.app/mcp",
    auth="<your-token>",
) as client:
    await client.ping()
```

Equivalent transport-level form:

```python id="nl6zwo"
from fastmcp import Client
from fastmcp.client.transports import StreamableHttpTransport

transport = StreamableHttpTransport(
    "https://your-server.fastmcp.app/mcp",
    auth="<your-token>",
)

async with Client(transport) as client:
    await client.ping()
```

If you want explicit control instead of implicit string-to-auth conversion, use `BearerAuth(token=...)`. The bearer-auth docs and SDK state that `BearerAuth` implements `httpx.Auth`. ([FastMCP][4])

```python id="w4lhsg"
from fastmcp import Client
from fastmcp.client.auth import BearerAuth

async with Client(
    "https://your-server.fastmcp.app/mcp",
    auth=BearerAuth(token="<your-token>"),
) as client:
    await client.ping()
```

If the server expects a non-Bearer header or custom token scheme, the docs recommend bypassing `auth=` entirely and setting headers directly on the transport. ([FastMCP][4])

```python id="a0drpc"
from fastmcp import Client
from fastmcp.client.transports import StreamableHttpTransport

async with Client(
    transport=StreamableHttpTransport(
        "https://your-server.fastmcp.app/mcp",
        headers={"X-API-Key": "<your-token>"},
    ),
) as client:
    await client.ping()
```

### 22.8.1 Bearer-auth guidance

Use raw-string `auth="<token>"` when brevity matters and the standard `Authorization: Bearer ...` scheme is correct. Use `BearerAuth(...)` when you want explicitness or when passing around auth objects. Use custom headers only when the server’s auth contract is non-standard. All three patterns are directly documented. ([FastMCP][4])

---

## 22.9 `auth="oauth"` and `OAuth(...)`

FastMCP’s OAuth client helper is the interactive browser-based path for OAuth 2.1 Authorization Code Flow with PKCE. The docs say OAuth auth is relevant only for HTTP transports, requires user interaction via a web browser, and that the simplest default configuration is `auth="oauth"`. ([FastMCP][5])

```python id="7z0uh6"
from fastmcp import Client

async with Client(
    "https://your-server.fastmcp.app/mcp",
    auth="oauth",
) as client:
    await client.ping()
```

For full control, instantiate `OAuth(...)` and pass it to `auth=` on the client or transport. The docs state that `OAuth` manages the full Authorization Code Grant + PKCE flow and implements `httpx.Auth`. They also state that when `OAuth` is used through `Client(auth=...)`, you do not need to pass `mcp_url`; the transport supplies the server URL automatically. ([FastMCP][5])

```python id="7ocfik"
from fastmcp import Client
from fastmcp.client.auth import OAuth

oauth = OAuth(scopes=["user"])

async with Client(
    "https://your-server.fastmcp.app/mcp",
    auth=oauth,
) as client:
    await client.ping()
```

### 22.9.1 `OAuth(...)` parameters that actually matter

The OAuth docs enumerate the main parameters:

* `scopes`: requested scopes
* `client_name`: name used for dynamic registration
* `client_id`: pre-registered OAuth client ID; when provided, DCR is skipped
* `client_secret`: optional secret for pre-registered clients
* `client_metadata_url`: URL-based client identity for CIMD
* `token_storage`: `AsyncKeyValue` backend for persisted tokens. ([FastMCP][5])

This yields three high-value patterns:

1. **Pure dynamic-registration client**: only `scopes` / `client_name`. ([FastMCP][5])
2. **Pre-registered client**: `client_id` (and optionally `client_secret`), which skips Dynamic Client Registration. ([FastMCP][5])
3. **CIMD client**: `client_metadata_url=...`, discussed below. ([FastMCP][5])

### 22.9.2 OAuth flow and token storage

The OAuth docs describe the flow as: check token storage for an existing valid token, discover the server’s OAuth endpoints via a well-known URI if no valid token exists, then run the browser/callback flow and exchange the authorization code for tokens. The same docs say the helper starts a temporary local HTTP callback server on an available port (or a configured callback port) and opens the user’s default browser. ([FastMCP][5])

Tokens are stored **in memory by default**, so they are lost on restart. The docs recommend supplying an `AsyncKeyValue` storage backend for persistence and explicitly advise using **encrypted storage in production** because a token store may accumulate credentials for many servers over time. ([FastMCP][5])

```python id="nhhe7u"
import os
from cryptography.fernet import Fernet
from key_value.aio.stores.disk import DiskStore
from key_value.aio.wrappers.encryption import FernetEncryptionWrapper
from fastmcp import Client
from fastmcp.client.auth import OAuth

encrypted_storage = FernetEncryptionWrapper(
    key_value=DiskStore(directory="~/.fastmcp/oauth-tokens"),
    fernet=Fernet(os.environ["OAUTH_STORAGE_ENCRYPTION_KEY"]),
)

oauth = OAuth(token_storage=encrypted_storage)

async with Client(
    "https://your-server.fastmcp.app/mcp",
    auth=oauth,
) as client:
    await client.ping()
```

### 22.9.3 OAuth guidance

Use `auth="oauth"` for the simplest interactive flow when defaults are acceptable. Use `OAuth(...)` whenever you need scopes, pre-registered credentials, CIMD, or persistent token storage. For production user-facing apps, persistent **encrypted** token storage should be considered the default. ([FastMCP][5])

---

## 22.10 CIMD via `OAuth(client_metadata_url=...)`

CIMD (Client ID Metadata Documents) is an OAuth enhancement for verifiable client identity. The CIMD docs say it is only relevant for HTTP transports and only works when the server advertises CIMD support. Instead of dynamically registering a fresh opaque `client_id` for every server, the client hosts a JSON metadata document at an HTTPS URL it controls, and that URL itself becomes the `client_id`. ([FastMCP][6])

FastMCP’s client-side entrypoint is simply `client_metadata_url=` on `OAuth(...)`: ([FastMCP][6])

```python id="jlwmmk"
from fastmcp import Client
from fastmcp.client.auth import OAuth

async with Client(
    "https://mcp-server.example.com/mcp",
    auth=OAuth(
        client_metadata_url="https://myapp.example.com/oauth/client.json",
    ),
) as client:
    await client.ping()
```

When the server supports CIMD, the docs say the client uses that metadata URL as its `client_id` and skips Dynamic Client Registration. The server fetches the document, validates it, and proceeds with the standard OAuth flow. ([FastMCP][6])

### 22.10.1 CIMD document requirements

The CIMD docs define the main hosting requirements:

* HTTPS only
* non-root path
* public accessibility
* `client_id` field must exactly match the hosting URL. ([FastMCP][6])

They also note that wildcard localhost redirect URIs such as `http://localhost:*/callback` are valid for development clients because FastMCP’s OAuth helper typically binds to an available local port for the callback server. ([FastMCP][6])

Use CIMD when you want the server and the user to see a **verifiable domain-backed client identity** instead of an arbitrary dynamically registered name. That is the exact value proposition articulated by the docs. ([FastMCP][6])

---

## 22.11 Handlers vs `message_handler`

The client constructor supports dedicated handlers plus a general `message_handler`. The notifications docs make the intended division explicit: while `message_handler` receives server-initiated messages, you should use **dedicated callbacks** for most interactive request types:

* `sampling_handler` for sampling requests
* `elicitation_handler` for user-input requests
* `progress_handler` for progress updates
* `log_handler` for log messages.

`message_handler` is primarily for monitoring and reacting to notifications. ([FastMCP][7])

This is a crucial design rule. Use dedicated handlers whenever FastMCP already has a typed callback surface for the protocol feature. Reserve `message_handler` for generic notification monitoring, cache invalidation, or unusual protocol events that do not merit a dedicated callback. ([FastMCP][7])

---

## 22.12 `sampling_handler`

The client sampling docs define the canonical handler signature as:

```python id="ix1z0q"
async def sampling_handler(
    messages: list[SamplingMessage],
    params: SamplingParams,
    context: RequestContext,
) -> str:
    ...
```

This handler responds when a server uses `ctx.sample()` / related sampling features to ask the client’s LLM to complete text during server-side execution. The docs say this is how servers delegate model reasoning back to the client, which controls which LLM is used and how requests are made. ([FastMCP][8])

`SamplingMessage` carries `role` (`"user"` or `"assistant"`) and content (`TextContent | ImageContent | AudioContent`). `SamplingParams` includes fields such as `systemPrompt`, `modelPreferences`, `temperature`, `maxTokens`, `stopSequences`, `tools`, and `toolChoice`. ([FastMCP][8])

```python id="ovggbu"
from fastmcp import Client
from fastmcp.client.sampling import SamplingMessage, SamplingParams, RequestContext

async def sampling_handler(
    messages: list[SamplingMessage],
    params: SamplingParams,
    context: RequestContext,
) -> str:
    conversation = []
    for message in messages:
        content = message.content.text if hasattr(message.content, "text") else str(message.content)
        conversation.append(f"{message.role}: {content}")

    system_prompt = params.systemPrompt or "You are a helpful assistant."
    # Call your own LLM here.
    return "Generated response"

client = Client("my_mcp_server.py", sampling_handler=sampling_handler)
```

Use `sampling_handler` when the client is expected to supply LLM capability to the server. Do not implement this through `message_handler` unless you have a very unusual reason; FastMCP already gives you a typed callback interface. ([FastMCP][8])

---

## 22.13 `elicitation_handler`

The client elicitation docs define the canonical handler signature as:

```python id="8ngs6u"
async def elicitation_handler(
    message: str,
    response_type: type | None,
    params: ElicitRequestParams,
    context: RequestContext,
) -> ElicitResult | object:
    ...
```

This handler runs when a server uses `ctx.elicit()` to request structured user input during an operation. The docs say FastMCP converts the server’s JSON schema into a Python dataclass type and passes that as `response_type`; if the server expects an empty object, `response_type` is `None`. Returning an ordinary object implicitly accepts the elicitation; returning an explicit `ElicitResult` gives full action control. ([FastMCP][9])

```python id="20fvva"
from fastmcp import Client
from fastmcp.client.elicitation import ElicitResult, ElicitRequestParams, RequestContext

async def elicitation_handler(
    message: str,
    response_type: type | None,
    params: ElicitRequestParams,
    context: RequestContext,
) -> ElicitResult | object:
    user_input = input(f"{message}: ")

    if not user_input:
        return ElicitResult(action="decline")

    return response_type(value=user_input) if response_type else ElicitResult(action="accept")

client = Client("my_mcp_server.py", elicitation_handler=elicitation_handler)
```

Use `elicitation_handler` when the client has an interactive human input surface. If the client is headless or non-interactive, do not provide this handler unless you have a machine-driven policy for responding to elicitation. ([FastMCP][9])

---

## 22.14 `log_handler`

The client logging docs define `log_handler` as the callback for server-sent log messages. The handler receives a `LogMessage` object with:

* `level`
* `logger`
* `data`, where `data` contains `msg` and `extra`. ([FastMCP][10])

```python id="zlg0kc"
import logging
from fastmcp import Client
from fastmcp.client.logging import LogMessage

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)
LOGGING_LEVEL_MAP = logging.getLevelNamesMapping()

async def log_handler(message: LogMessage):
    msg = message.data.get("msg")
    extra = message.data.get("extra")
    level = LOGGING_LEVEL_MAP.get(message.level.upper(), logging.INFO)
    logger.log(level, msg, extra=extra)

client = Client("my_mcp_server.py", log_handler=log_handler)
```

The docs also emphasize that `message.data` is structured and remains structured even when logs pass through a FastMCP proxy, which makes it useful for debugging multi-server systems. ([FastMCP][10])

If you do **not** provide a custom `log_handler`, FastMCP’s default handler routes server logs into Python’s logging system at appropriate severities. The docs explicitly note mappings such as `notice -> INFO` and `alert` / `emergency -> CRITICAL`. ([FastMCP][11])

Use a custom `log_handler` when you want structured forwarding into your own logging/observability system; otherwise the default behavior is often sufficient. ([FastMCP][11])

---

## 22.15 `progress_handler`

The progress docs define `progress_handler` as the callback for long-running-operation progress. Its signature is:

```python id="y3ud1q"
async def progress_handler(
    progress: float,
    total: float | None,
    message: str | None,
) -> None:
    ...
```

The handler receives current progress, optional total, and optional message. The docs also show that `progress_handler` can be set at the client level and overridden per `call_tool(...)` invocation. ([FastMCP][12])

```python id="m4hr48"
from fastmcp import Client

async def progress_handler(
    progress: float,
    total: float | None,
    message: str | None,
) -> None:
    if total is not None:
        percentage = (progress / total) * 100
        print(f"Progress: {percentage:.1f}% - {message or ''}")
    else:
        print(f"Progress: {progress} - {message or ''}")

client = Client("my_mcp_server.py", progress_handler=progress_handler)

# Per-call override
async with client:
    result = await client.call_tool(
        "long_running_task",
        {"param": "value"},
        progress_handler=progress_handler,
    )
```

If no custom handler is provided, the SDK exposes `default_progress_handler(...)`, which logs progress at debug level. Use a custom handler whenever progress needs to drive a UI, CLI progress display, or metrics sink. ([FastMCP][12])

---

## 22.16 Roots: static and dynamic

Roots let the client tell the server what local resources or filesystem boundaries it can provide. The roots docs define two supported patterns:

* a static list of roots
* a dynamic callback that computes roots when requested. ([FastMCP][13])

Static roots:

```python id="jlwmzo"
from fastmcp import Client

client = Client(
    "my_mcp_server.py",
    roots=["/path/to/root1", "/path/to/root2"],
)
```

Dynamic roots:

```python id="pi9qgc"
from fastmcp import Client
from fastmcp.client.roots import RequestContext

async def roots_callback(context: RequestContext) -> list[str]:
    print(f"Server requested roots (Request ID: {context.request_id})")
    return ["/path/to/root1", "/path/to/root2"]

client = Client(
    "my_mcp_server.py",
    roots=roots_callback,
)
```

Use static roots when the local resource boundary is fixed and stable. Use dynamic roots when root availability depends on the request, current workspace, or runtime policy. The roots SDK also exposes helper functions like `create_roots_callback(...)`, but the high-level behavior is already captured by `roots=` on `Client(...)`. ([FastMCP][13])

---

## 22.17 Notifications and `message_handler`

The notifications docs define `message_handler` as the unified notification-monitoring callback. The simplest form is a function receiving each protocol message and filtering for notification types such as:

* `notifications/tools/list_changed`
* `notifications/resources/list_changed`
* `notifications/prompts/list_changed`. ([FastMCP][7])

```python id="hpjlwm"
from fastmcp import Client

async def message_handler(message):
    if hasattr(message, "root"):
        method = message.root.method
        if method == "notifications/tools/list_changed":
            print("Tools changed")
        elif method == "notifications/resources/list_changed":
            print("Resources changed")
        elif method == "notifications/prompts/list_changed":
            print("Prompts changed")

client = Client("my_mcp_server.py", message_handler=message_handler)
```

For finer-grained dispatch, subclass `fastmcp.client.messages.MessageHandler`. The docs show hook-style methods such as `on_tool_list_changed(...)`, which is the right pattern when you want structured notification routing rather than one large filter function. ([FastMCP][7])

```python id="o2rw8c"
from fastmcp import Client
from fastmcp.client.messages import MessageHandler
import mcp.types

class MyMessageHandler(MessageHandler):
    async def on_tool_list_changed(
        self,
        notification: mcp.types.ToolListChangedNotification,
    ) -> None:
        print("Tool list changed")

client = Client("my_mcp_server.py", message_handler=MyMessageHandler())
```

Use `message_handler` for notifications and protocol monitoring. Do **not** treat it as the primary interface for sampling, elicitation, progress, or logging when dedicated handlers already exist. The docs explicitly recommend the dedicated callbacks for those cases. ([FastMCP][7])

---

## 22.18 Best-practice guidance

Use **`StreamableHttpTransport`** for all new production HTTP deployments. The docs explicitly call HTTP/Streamable HTTP the recommended production transport. ([FastMCP][3])

Use **`SSETransport`** only for backward compatibility or specific infrastructure constraints. The docs explicitly say to prefer Streamable HTTP for new deployments. ([FastMCP][3])

When using **STDIO**, always pass `env=...` explicitly for anything the server needs. Do not assume shell variables propagate. Prefer explicit `StdioTransport(...)` over file-path inference for any serious local deployment. ([FastMCP][3])

Use **in-memory transport** for tests and evaluation harnesses when same-process behavior is desirable. Unlike STDIO, it shares environment and memory with the client. ([FastMCP][3])

Use `auth="<token>"` or `BearerAuth(...)` for non-interactive bearer tokens, `auth="oauth"` for the simplest interactive OAuth flow, `OAuth(...)` when you need scopes/storage/pre-registered credentials/CIMD, and `client_metadata_url=...` when you want CIMD-backed verifiable client identity on a supporting server. ([FastMCP][4])

Use **dedicated handlers** for server-initiated interactive capabilities—sampling, elicitation, progress, and logging. Use `message_handler` primarily for notifications and generic protocol observation. ([FastMCP][7])


[1]: https://gofastmcp.com/python-sdk/fastmcp-client-client "client - FastMCP"
[2]: https://gofastmcp.com/clients/client "The FastMCP Client - FastMCP"
[3]: https://gofastmcp.com/clients/transports "Client Transports - FastMCP"
[4]: https://gofastmcp.com/clients/auth/bearer "Bearer Token Authentication - FastMCP"
[5]: https://gofastmcp.com/clients/auth/oauth "OAuth Authentication - FastMCP"
[6]: https://gofastmcp.com/clients/auth/cimd "CIMD Authentication - FastMCP"
[7]: https://gofastmcp.com/clients/notifications "Notifications - FastMCP"
[8]: https://gofastmcp.com/clients/sampling "LLM Sampling - FastMCP"
[9]: https://gofastmcp.com/clients/elicitation "User Elicitation - FastMCP"
[10]: https://gofastmcp.com/clients/logging "Server Logging - FastMCP"
[11]: https://gofastmcp.com/clients/logging "Server Logging"
[12]: https://gofastmcp.com/clients/progress "Progress Monitoring - FastMCP"
[13]: https://gofastmcp.com/clients/roots "Client Roots - FastMCP"

# FastMCP Advanced — 23) Client-only packaging and `fastmcp-remote`

## 23.0 Two different deployment problems

FastMCP 3.3 and 3.4 added two packaging/runtime tools that solve different problems:

```text
fastmcp-slim[client]
    -> Python code wants FastMCP Client/transport functionality
       without installing the full server stack

fastmcp-remote
    -> an MCP host only knows how to spawn STDIO servers,
       but the real MCP endpoint is remote HTTP
```

Do not conflate package footprint optimization with transport bridging. ([FastMCP Updates][1])

---

## 23.1 `fastmcp-slim[client]`

Install:

```bash
pip install "fastmcp-slim[client]"
```

Import namespace remains:

```python
from fastmcp import Client
```

Use it for framework/library code that connects to MCP servers but does not construct/run FastMCP servers itself. The full `fastmcp` package remains the normal choice for servers and Apps. ([Client-Only Package][2])

---

## 23.2 Why slim packaging matters

Benefits:

* smaller dependency graph;
* avoids server-only ASGI/runtime dependencies in client libraries;
* lower conflict risk when embedding MCP connectivity into another framework;
* cleaner responsibility boundary;
* potentially faster environment creation/cold import.

Tradeoff: code may later need a server-side feature and discover that the slim environment intentionally lacks it. Keep client and server dependency groups explicit.

---

## 23.3 Library dependency pattern

For a package whose public API only consumes MCP:

```toml
[project]
dependencies = [
  "fastmcp-slim[client]",
]
```

Application code:

```python
from fastmcp import Client


async def fetch_tools(url: str):
    async with Client(url) as client:
        return await client.list_tools()
```

Avoid forcing all downstream users to install the full FastMCP server stack when your library never runs a server.

---

## 23.4 `fastmcp-remote` role

`fastmcp-remote` is a standalone stdio bridge.

```text
MCP host
   STDIO
     |
     v
fastmcp-remote process
     |
     | Streamable HTTP (+ OAuth/headers)
     v
remote MCP endpoint
```

FastMCP 3.4.0 introduced this package as a standard bridge pattern. ([fastmcp-remote][3])

---

## 23.5 Minimal bridge invocation

```bash
uvx fastmcp-remote https://example.com/mcp
```

If installed persistently:

```bash
uv tool install fastmcp-remote
fastmcp-remote https://example.com/mcp
```

Pass the full MCP endpoint URL. A common error is providing `https://example.com` when the server actually listens at `https://example.com/mcp`. ([fastmcp-remote][3])

---

## 23.6 Host JSON pattern

```json
{
  "mcpServers": {
    "remote-api": {
      "command": "uvx",
      "args": [
        "fastmcp-remote",
        "https://example.com/mcp"
      ]
    }
  }
}
```

This pattern is useful for hosts whose MCP configuration model assumes every server is a local command even when the actual service is remote.

---

## 23.7 OAuth behavior

The remote bridge supports OAuth for remote endpoints. In typical usage, the first authenticated connection can launch a browser flow and cache tokens locally for subsequent runs. ([fastmcp-remote][3])

Operational questions to answer:

* where tokens are cached;
* filesystem permissions on the cache;
* whether the host runs under the same OS user each time;
* how token revocation/logout is performed;
* whether headless environments can complete browser auth;
* whether corporate proxies/custom CAs are needed.

---

## 23.8 Explicit Authorization header

For pre-provisioned bearer credentials:

```bash
uvx fastmcp-remote \
  https://example.com/mcp \
  --header "Authorization: Bearer $MCP_TOKEN"
```

An explicit Authorization header typically changes/disables automatic OAuth behavior because the bridge already has caller-provided auth material. See exact CLI help for the installed bridge version. ([fastmcp-remote][3])

---

## 23.9 Multiple headers

Repeat the header option when the remote endpoint needs additional gateway metadata:

```bash
uvx fastmcp-remote \
  https://example.com/mcp \
  --header "Authorization: Bearer $MCP_TOKEN" \
  --header "X-Tenant: tenant-a"
```

Security rule: do not put long-lived secrets directly into checked-in JSON if the host supports environment/secret substitution.

---

## 23.10 Bridge initialization must fail loudly

A 3.4 design improvement forwards initialization upstream. Wrong URL, unreachable backend, or upstream initialization/auth failure should fail bridge initialization rather than appearing as an empty connected server. ([FastMCP Updates][1])

Use this behavior in health/diagnostic tooling:

```text
"bridge process exists" != "remote MCP is usable"
```

---

## 23.11 `fastmcp-remote` vs programmatic `Client`

Use `Client(url)` when you control Python application code:

```python
async with Client("https://example.com/mcp") as client:
    result = await client.call_tool(...)
```

Use `fastmcp-remote` when a third-party host insists on a STDIO command configuration.

Adding an extra process between your own Python client and a remote endpoint is usually unnecessary.

---

## 23.12 `fastmcp-remote` vs custom ProxyProvider gateway

Use remote bridge when:

```text
one remote backend
host requires STDIO
minimal policy/transformation needed
```

Use a custom FastMCP proxy/gateway when:

```text
multiple upstreams
namespacing/transforms
custom auth/authorization
middleware/audit/rate limits
stable gateway endpoint
custom health/readiness
```

---

## 23.13 Child process environment

STDIO hosts may launch the bridge with a restricted environment. If auth depends on env vars, CA paths, proxy configuration, or browser commands, verify what the host actually passes to the child.

Do not diagnose OAuth failures only from a terminal invocation if the real host process has a different environment.

---

## 23.14 Endpoint diagnostics

When the bridge fails:

1. `curl`/HTTP reachability alone is insufficient—verify the endpoint speaks MCP.
2. Check `/mcp` vs root path.
3. Check TLS/CA/proxy behavior.
4. Check auth discovery/redirect URLs.
5. Run `fastmcp-remote` manually under the same OS user.
6. Inspect stderr/log output from the host-launched process.
7. Verify cached OAuth credentials are readable and not stale.

---

## 23.15 Version alignment

`fastmcp-remote` is a separate package and can evolve independently. Record its version alongside the server's FastMCP version in reproducible host configs.

Example wrapper script:

```bash
#!/usr/bin/env bash
exec uvx --from "fastmcp-remote==<pinned-version>" \
  fastmcp-remote "https://example.com/mcp"
```

Use the actual current package pin from your lock/config; do not copy a placeholder version from this reference.

---

## 23.16 Security posture for desktop bridges

A desktop bridge may persist tokens on a user workstation. Apply normal local-secret hygiene:

* user-only file permissions;
* OS disk encryption where required;
* no token values in debug logs;
* clean logout/revocation process;
* avoid shared OS accounts;
* use short-lived/refreshable tokens;
* scope remote permissions to the MCP capability set.

---

## 23.17 Agent integration pattern

For an LLM coding agent that can execute shell commands but cannot natively connect to remote MCP:

```text
agent shell
  -> fastmcp CLI / fastmcp-remote
  -> remote MCP
```

Prefer `fastmcp list`/`call` or generated CLI workflows when the agent only needs occasional deterministic operations; prefer a persistent MCP client integration when it needs rich session/progress/notification behavior.

---

## 23.18 Anti-pattern inventory

* installing full FastMCP in every downstream client library without need;
* importing from a fictitious `fastmcp_slim` namespace;
* running `fastmcp-remote` between your own Python `Client` and the server;
* giving the bridge the host root instead of full MCP path;
* committing bearer tokens into host JSON;
* assuming bridge process success means upstream initialization succeeded;
* relying on shell OAuth/cache environment that the actual desktop host does not inherit;
* using a simple remote bridge when policy requires a real gateway;
* leaving token cache permissions/world-readable state unreviewed.

---

## 23.19 Agent checklist

```text
[ ] Client-only Python package? Consider fastmcp-slim[client].
[ ] Server process? Use full fastmcp package.
[ ] STDIO-only host connecting to remote HTTP? Use fastmcp-remote.
[ ] Pass full /mcp endpoint URL.
[ ] Decide OAuth vs explicit Authorization header.
[ ] Keep secrets out of checked-in host JSON.
[ ] Verify child-process environment and token cache permissions.
[ ] Pin bridge/package versions for reproducible deployments.
[ ] Use a custom ProxyProvider gateway when transforms/policy/multiple upstreams are needed.
```

[1]: https://gofastmcp.com/updates "FastMCP Updates"
[2]: https://gofastmcp.com/clients/client-only-package "Client-Only Package"
[3]: https://gofastmcp.com/clients/fastmcp-remote "fastmcp-remote"

# FastMCP Advanced — 24) Apps and interactive UI delivery
### Apps and interactive UI delivery

### 24.0 Apps as a separate product surface

FastMCP Apps are not “just prettier tool outputs.” They are a distinct product surface layered on MCP tools: a tool returns UI metadata and structured payload, the host renders that payload inside a sandboxed iframe, and UI interactions can call back into the server through the MCP Apps extension. The architecture docs compress the full path as `Python components → JSON tree → structuredContent → Renderer iframe → Host UI`, with host/iframe communication handled through `postMessage` and the `@modelcontextprotocol/ext-apps` AppBridge SDK. ([gofastmcp.com][1])

The `fastmcp.apps` package is the dedicated API surface for this layer. The SDK package index defines it as containing `FastMCPApp`, `AppConfig`, and the security primitives `ResourceCSP` and `ResourcePermissions`. That is the cleanest boundary: plain `FastMCP` gives you server components; `fastmcp.apps` gives you app-specific composition, UI configuration, and security metadata. ([gofastmcp.com][2])

### 24.1 Decision tree: which app architecture to choose

The official overview gives a crisp four-way decision tree. Start with **Prefab Apps**—`@mcp.tool(app=True)` plus a Prefab return—when you want charts, tables, dashboards, or client-side interactivity without significant server-side tool orchestration. Move to **FastMCPApp** when the UI needs multiple backend tools, managed visibility, and composition-safe tool routing. Choose **GenerativeUI** when the LLM should synthesize the UI at runtime instead of calling prebuilt tools with fixed interfaces. Drop to **Custom HTML Apps** when you need your own HTML/CSS/JavaScript, a specific frontend framework, or capabilities like maps, 3D, or video that are outside Prefab’s component model. ([gofastmcp.com][3])

A good operational heuristic is therefore: `app=True` for display-first tools, `FastMCPApp` for server-connected applications, `GenerativeUI` for model-authored UIs, and low-level `AppConfig + ui://resource` HTML when you need full renderer control. The docs state that FastMCP also ships ready-made app providers, but those sit orthogonally to this decision tree—they are convenience providers on top of the same app surface. ([gofastmcp.com][3])

### 24.2 Prefab Apps: `@mcp.tool(app=True)` as the simplest UI path

The shortest app path is a normal tool decorated with `app=True` that returns a Prefab UI object such as `PrefabApp`. The Prefab docs and Apps overview both state that this is the simplest way to turn a tool into an interactive UI: the host renders a UI instead of plain JSON/text, and this supports everything from static displays to reactive dashboards with client-side state and actions. ([gofastmcp.com][4])

```python id="n8m3a4"
from prefab_ui.app import PrefabApp
from prefab_ui.components import Column, Heading
from prefab_ui.components.charts import BarChart, ChartSeries
from fastmcp import FastMCP

mcp = FastMCP("Dashboard")

@mcp.tool(app=True)
def revenue_chart(year: int) -> PrefabApp:
    data = [
        {"quarter": "Q1", "revenue": 42000},
        {"quarter": "Q2", "revenue": 51000},
        {"quarter": "Q3", "revenue": 47000},
        {"quarter": "Q4", "revenue": 63000},
    ]
    with Column(gap=4, css_class="p-6") as view:
        Heading(f"{year} Revenue")
        BarChart(
            data=data,
            series=[ChartSeries(data_key="revenue", label="Revenue")],
            x_axis="quarter",
        )
    return PrefabApp(view=view)
```

When the host calls such a tool, FastMCP serializes the Prefab component tree to JSON, places it into `structuredContent`, and the shared Prefab renderer paints it in the iframe. By default, the tool still emits a small text `content` payload—`"[Rendered Prefab UI]"`—so the LLM knows something was rendered; if you need to control both the text half and the UI half, return an explicit `ToolResult`. ([gofastmcp.com][4])

Use `app=True` when the UI is mostly self-contained and server round-trips are minimal or optional. The docs explicitly note that there is “no hard wall” stopping a Prefab app from calling other tools via `CallTool`, but they also frame `FastMCPApp` as the better abstraction once backend interaction becomes nontrivial. ([gofastmcp.com][3])

### 24.3 Production caveat: `prefab-ui` must be pinned by you

This is the single most important production warning on the Apps surface. Both the overview, Prefab page, and FastMCPApp page state that Prefab is in early active development, that FastMCP pins only a **minimum** compatible `prefab-ui` version, and that it **does not pin an upper bound**. The docs explicitly say that production deployments should pin `prefab-ui` to a specific version in your own dependencies, because an unpinned fresh deploy can resolve a newer breaking Prefab release and break your UI. ([gofastmcp.com][3])

The correct production policy is therefore: pin `fastmcp`, pin `prefab-ui`, and treat the app surface as a versioned application dependency set rather than assuming FastMCP alone constrains all runtime UI dependencies. ([gofastmcp.com][3])

### 24.4 `FastMCPApp`: managed app composition for server-connected UIs

`FastMCPApp` is a **Provider**, not a different server type. The SDK defines it as a composable MCP application provider that binds together entry-point tools (`@app.ui()`), backend tools (`@app.tool()`), and the Prefab renderer resource. The interactive-app guide positions it specifically for apps with heavy server interaction, where plain `app=True` plus string-based `CallTool("...")` references become fragile under namespacing and growth. ([gofastmcp.com][5])

The documented attachment pattern is provider-based:

```python id="v6s7u1"
from fastmcp import FastMCP, FastMCPApp

app = FastMCPApp("Contacts")

@app.ui()
def contact_manager():
    ...

@app.tool()
def save_contact(name: str, email: str):
    ...

mcp = FastMCP("Platform", providers=[app])
# or:
# mcp = FastMCP("Platform")
# mcp.add_provider(app)
```

This is the correct object-model boundary: `FastMCPApp` is added to a server the same way any other provider is added. ([gofastmcp.com][6])

### 24.5 `@app.ui()` — model-visible entry points

`@app.ui()` registers entry-point tools that the **model** calls to open the app. The interactive-app docs say these return Prefab UI, default to `visibility=["model"]`, auto-wire the shared Prefab renderer resource and CSP, and support the same major options as `@mcp.tool`, including `name`, `description`, `title`, `tags`, `icons`, `auth`, and `timeout`. ([gofastmcp.com][6])

```python id="3el9kg"
from prefab_ui.app import PrefabApp
from prefab_ui.components import Column, Heading
from fastmcp import FastMCPApp

app = FastMCPApp("Dashboard")

@app.ui(title="Contact Manager", description="Open the contact management interface")
def contact_manager() -> PrefabApp:
    with Column(gap=4, css_class="p-6") as view:
        Heading("Contacts")
    return PrefabApp(view=view)
```

Use `@app.ui()` when the app should appear in the model-visible tool list as the entry point into a larger UI flow. The model sees the entry tool; it does not need to see every internal mutation/search/save backend operation. ([gofastmcp.com][6])

### 24.6 `@app.tool()` — app-visible backend tools

`@app.tool()` registers backend tools for the UI to call via `CallTool`. The docs say these tools default to `visibility=["app"]`, are hidden from the model by default, and support `name`, `description`, `auth`, and `timeout`. Passing `model=True` exposes them to both the model and the app UI. ([gofastmcp.com][6])

```python id="5z4l1o"
from fastmcp import FastMCPApp

app = FastMCPApp("Contacts")
db: list[dict] = []

@app.tool()
def save_contact(name: str, email: str) -> list[dict]:
    db.append({"name": name, "email": email})
    return list(db)

@app.tool(model=True)
def list_contacts() -> list[dict]:
    return list(db)
```

The separation is deliberate: entry points open the app; backend tools do the work. Use `model=True` only when the same operation is genuinely useful both as a conversational tool and as an in-app action. Otherwise keep backend tools app-only to reduce model-visible catalog noise. ([gofastmcp.com][6])

### 24.7 Why `FastMCPApp` exists: stable backend routing and visibility discipline

The main value of `FastMCPApp` is not “it can call tools”; ordinary `app=True` Prefab apps can already do that. The value is **managed binding**: backend tools get stable identifiers that survive namespacing, visibility is automatically partitioned between model-visible and app-visible tools, and `CallTool` can accept **function references** instead of raw strings, making internal tool references refactorable and composition-safe. The docs explain this explicitly. ([gofastmcp.com][6])

The architecture docs reveal how this works. `FastMCPApp` tags both entry points and backend tools with `meta["fastmcp"]["app"]`; during serialization it injects `_meta.fastmcp.app` into `structuredContent`; when the renderer later calls a backend tool, the server sees that tag and uses `get_app_tool(app_name, tool_name)` instead of normal `get_tool(name)`. That bypasses the normal transform chain, so a backend tool like `save_contact` still resolves correctly even if the parent server mounted the app under a namespace and transformed the model-visible tool name. Authorization still runs. ([gofastmcp.com][1])

That makes `FastMCPApp` the correct abstraction whenever your UI has multiple backend calls, may be mounted under namespaces, or must survive refactors without brittle string tool-name coupling. ([gofastmcp.com][6])

### 24.8 `CallTool(...)`, result handling, and `result_key`

The interactive-app docs define `CallTool` as the bridge between the UI and server. It can accept either a tool name string or a backend tool function reference, and function references are the safer form when the tool is defined in the same file because FastMCP resolves them to stable backend identifiers automatically. The same docs also document `result_key` as shorthand for “store the successful result into this state key.” ([gofastmcp.com][6])

```python id="s8b6ry"
from prefab_ui.actions import SetState, ShowToast
from prefab_ui.actions.mcp import CallTool
from prefab_ui.rx import RESULT, STATE

# Function reference form is composition-safe
CallTool(
    save_contact,
    arguments={"name": STATE.name, "email": STATE.email},
    on_success=[
        SetState("contacts", RESULT),
        ShowToast("Saved"),
    ],
)

# result_key shorthand
CallTool("list_contacts", result_key="contacts")
```

Use string names only when the backend tool reference is not directly importable in the UI definition site. Prefer function references when possible; the docs explicitly position them as the refactor-safe/composition-safe form. ([gofastmcp.com][6])

### 24.9 `FastMCPApp.run()` — standalone development path

`FastMCPApp` exposes a `run(...)` convenience method that wraps the app in a temporary `FastMCP` server and runs it standalone. The interactive-app docs and SDK both document this as a development convenience, not a fundamentally different deployment mode. ([gofastmcp.com][6])

```python id="7vtgtg"
from fastmcp import FastMCPApp

app = FastMCPApp("Contacts")

# ... register @app.ui() / @app.tool() ...

if __name__ == "__main__":
    app.run()
```

Use `app.run()` for quick standalone development when the app is the only thing you need to serve. Use `providers=[app]` / `add_provider(app)` when the app is one capability surface inside a larger platform server. ([gofastmcp.com][6])

### 24.10 `GenerativeUI()` — runtime LLM-authored Prefab UIs

`GenerativeUI` is a **Provider** that lets the model generate Prefab Python code at runtime and render it as a streaming UI. The docs say it registers three things: a generation tool (`generate_prefab_ui` in the high-level docs, `generate_ui` in the SDK reference naming), a component-search tool, and a renderer resource using browser-side Pyodide for streaming. The high-level provider docs describe the registered public tool names as `generate_prefab_ui` and `search_prefab_components`; the SDK reference describes the internal capability as `generate_ui` plus `components`, but both agree on the functional model: code generation tool + component search tool + renderer resource. ([gofastmcp.com][7])

```python id="jlwmqd"
from fastmcp import FastMCP
from fastmcp.apps.generative import GenerativeUI

mcp = FastMCP("Prefab Studio")
mcp.add_provider(GenerativeUI())
```

The provider docs state that the LLM writes real Python using Prefab’s component library, that partial code is streamed to the renderer via `ontoolinputpartial`, and that the user watches the UI build up in real time. Configuration options include renaming the generation tool, renaming the component-search tool, and omitting the search tool entirely. ([gofastmcp.com][7])

Use `GenerativeUI` when the UI shape itself should be model-authored at runtime rather than fixed in code. Do **not** use it when you need a strongly versioned, carefully curated UI contract—use Prefab/FastMCPApp/custom HTML in those cases. That recommendation is an engineering conclusion directly supported by the docs’ framing of GenerativeUI as “let the LLM generate custom UIs at runtime.” ([gofastmcp.com][7])

The provider docs also give concrete operational constraints: `fastmcp[apps]` is required, a Pyodide validation sandbox runs server-side and needs Deno on first use, the browser-side renderer loads Pyodide from a CDN with CSP configured automatically, and the sandbox includes only the standard library plus Prefab—not arbitrary external Python packages like NumPy or pandas. Those are real deployment constraints, not incidental details. ([gofastmcp.com][7])

### 24.11 Custom HTML Apps via the MCP Apps extension

When Prefab is the wrong abstraction, drop to the MCP Apps extension directly. The low-level apps docs define the custom HTML model as two artifacts: a tool that returns data and a `ui://...` resource containing the HTML that renders that data. The tool links to the resource via `AppConfig(resource_uri=...)`, the host fetches the linked UI resource, renders it inside a sandboxed iframe, and pushes the tool result into it via `postMessage`. ([gofastmcp.com][8])

Canonical low-level pattern:

```python id="mnnxpp"
import json
from fastmcp import FastMCP
from fastmcp.apps import AppConfig

mcp = FastMCP("My App Server")

@mcp.tool(app=AppConfig(resource_uri="ui://my-app/view.html"))
def generate_chart(data: list[float]) -> str:
    return json.dumps({"values": data})

@mcp.resource("ui://my-app/view.html")
def chart_view() -> str:
    return "<html>...</html>"
```

This is the right path when you need custom rendering stacks, maps, 3D, video, or a JavaScript framework not represented naturally as Prefab components. The docs explicitly position it that way. ([gofastmcp.com][8])

### 24.12 `AppConfig`: the low-level app wiring object

`AppConfig` is the low-level configuration object for how a tool or resource participates in the Apps extension. On tools, the main field is `resource_uri`; on tools and resources, additional fields include `csp`, `permissions`, `domain`, and `prefers_border`; and on tools only, `visibility` controls whether the tool is visible to the model, only the app, or both. The docs also say you can pass a raw dict with camelCase wire-format keys instead of an `AppConfig` instance. ([gofastmcp.com][8])

The documented fields are:

* `resource_uri`: UI resource URI, tools only
* `visibility`: `["model"]`, `["app"]`, or both, tools only
* `csp`: `ResourceCSP` for iframe CSP
* `permissions`: `ResourcePermissions` for iframe sandbox/browser capabilities
* `domain`: stable sandbox origin
* `prefers_border`: UI border preference. ([gofastmcp.com][8])

On resources, `resource_uri` and `visibility` must **not** be set, because the resource **is** the UI. The docs state that clearly. ([gofastmcp.com][8])

### 24.13 `ui://` resources and MIME semantics

Resources using the `ui://` scheme are automatically served with MIME type `text/html;profile=mcp-app`. The custom HTML docs say you do not need to set this manually. The host renders these resources in a sandboxed iframe and opens an AppBridge communication channel back to the host/server. ([gofastmcp.com][8])

```python id="jzru1i"
@mcp.resource("ui://my-app/view.html")
def my_view() -> str:
    return "<html><body>Hello</body></html>"
```

This `ui://` resource model is one of the most important boundaries in the app system: tools do work and return data; `ui://` resources render that data. Once you drop to custom HTML, think in those two artifacts explicitly. ([gofastmcp.com][8])

### 24.14 Security: CSP and permissions for custom HTML

Custom HTML apps run in sandboxed iframes with a deny-by-default Content Security Policy. The docs say that by default only inline scripts/styles are allowed and there is no external network access. If the UI needs CDN assets, API requests, or nested frames, those domains must be declared through `ResourceCSP`. ([gofastmcp.com][8])

```python id="hi2b0k"
from fastmcp.apps import AppConfig, ResourceCSP

@mcp.resource(
    "ui://my-app/view.html",
    app=AppConfig(
        csp=ResourceCSP(
            resource_domains=["https://unpkg.com", "https://cdn.example.com"],
            connect_domains=["https://api.example.com"],
        )
    ),
)
def my_view() -> str:
    return "<html>...</html>"
```

The documented CSP field meanings are:

* `connect_domains` for `fetch` / XHR / WebSocket
* `resource_domains` for scripts/images/styles/fonts
* `frame_domains` for nested iframes
* `base_uri_domains` for document `base-uri`. ([gofastmcp.com][8])

Browser capabilities such as camera or clipboard access are requested through `ResourcePermissions`, but the docs explicitly warn that hosts may or may not grant them, so your UI should use feature detection/fallbacks rather than assuming success. ([gofastmcp.com][8])

```python id="l7v6ns"
from fastmcp.apps import AppConfig, ResourcePermissions

@mcp.resource(
    "ui://my-app/view.html",
    app=AppConfig(
        permissions=ResourcePermissions(
            camera={},
            clipboard_write={},
        )
    ),
)
def my_view() -> str:
    return "<html>...</html>"
```

### 24.15 Host support detection and fallback behavior

Not all MCP hosts support the Apps extension. The custom HTML docs explicitly recommend checking support at runtime using `ctx.client_supports_extension(UI_EXTENSION_ID)` and falling back to plain-text or non-app responses when necessary. This is equally relevant for Prefab and custom HTML, even though the example is shown on the low-level page. ([gofastmcp.com][8])

```python id="r4jns2"
from fastmcp import FastMCP, Context
from fastmcp.apps import AppConfig, UI_EXTENSION_ID

mcp = FastMCP("Adaptive Apps")

@mcp.tool(app=AppConfig(resource_uri="ui://my-app/view.html"))
async def my_tool(ctx: Context) -> str:
    if ctx.client_supports_extension(UI_EXTENSION_ID):
        return "rich-response"
    return "plain-text-fallback"
```

Use this pattern whenever the server must behave reasonably in both app-capable and non-app-capable clients. Treat app support as a runtime capability check, not as a universal assumption. ([gofastmcp.com][8])

### 24.16 Local previewing with `fastmcp dev apps`

`fastmcp dev apps ...` is the official local preview environment for app tools. The development docs say it launches a browser-based preview with no MCP host client required, works for both Prefab apps and custom HTML apps, starts your MCP server and a local dev UI side by side, auto-generates input forms from the tool schema, and opens rendered results in a new tab. ([gofastmcp.com][9])

```bash id="z1ek7x"
fastmcp dev apps server.py
fastmcp dev apps server.py:mcp --mcp-port 9000 --dev-port 9090 --no-reload
```

The default ports are documented as MCP server on `8000` and dev UI on `8080`, with auto-reload on by default. The dev UI also includes an inspector panel that shows JSON-RPC and AppBridge traffic in real time, which is especially useful for debugging app tool arguments, returned `structuredContent`, and UI-to-server `CallTool` traffic. ([gofastmcp.com][9])

Architecturally, the dev server runs two HTTP servers and a reverse proxy at `/mcp`. The architecture docs explain why: the renderer iframe runs on the dev UI origin, while your MCP server runs on another port, so the reverse proxy makes `callServerTool` same-origin and avoids browser CORS blocking. That makes `fastmcp dev apps` a reasonably faithful host simulation rather than a toy preview. ([gofastmcp.com][9])

### 24.17 Production guidance

Use plain `@mcp.tool(app=True)` plus Prefab returns for **display-first** tools or small self-contained UIs. Use `FastMCPApp` for **server-connected applications** with multiple backend calls, visibility partitioning, and composition-safe tool routing. Use `GenerativeUI()` when the **model should design the UI**, not when you need a fixed audited interface. Use Custom HTML / `AppConfig(resource_uri=...)` when you need **full frontend control**, custom JavaScript frameworks, or browser features beyond Prefab’s component model. This four-way split is directly supported by the official Apps overview. ([gofastmcp.com][3])

Pin `prefab-ui` in production. Treat `fastmcp dev apps` as the local preview/debug loop, not as a deployment topology. Treat host app-support checks as a real capability boundary and implement fallbacks when the same tool must behave well in non-app MCP clients. And when building custom HTML apps, treat CSP and permissions as production configuration, not optional polish, because the default runtime is sandboxed and deny-by-default. ([gofastmcp.com][3])


[1]: https://gofastmcp.com/apps/architecture "App Architecture - FastMCP"
[2]: https://gofastmcp.com/python-sdk/fastmcp-apps-__init__ "__init__ - FastMCP"
[3]: https://gofastmcp.com/apps/overview "Apps - FastMCP"
[4]: https://gofastmcp.com/apps/prefab "Prefab UI - FastMCP"
[5]: https://gofastmcp.com/python-sdk/fastmcp-apps-app "app - FastMCP"
[6]: https://gofastmcp.com/apps/interactive-apps "FastMCPApp - FastMCP"
[7]: https://gofastmcp.com/apps/providers/generative "Generative UI - FastMCP"
[8]: https://gofastmcp.com/apps/low-level "Custom HTML Apps - FastMCP"
[9]: https://gofastmcp.com/apps/development "Development - FastMCP"

# FastMCP Advanced — 25) Prefab, built-in app providers, Generative UI, and custom renderers
### Prefab, built-in app providers, Generative UI, and custom renderers

### 25.0 Scope and version frame

FastMCP 3.2 made Apps a first-class stable product surface rather than an experimental side channel. The stable 3.4.x line includes the Prefab renderer path, `FastMCPApp`, built-in app providers for approval/choice/forms/files, Generative UI, and the lower-level MCP Apps resource model. Section 24 explains the architecture; this section is the implementation catalog and production decision guide.

The core invariant is:

```text
model-visible tool
  -> tool call
      -> normal MCP result content
      -> structuredContent + UI metadata
          -> host loads ui:// renderer in sandboxed iframe
              -> UI may call app-visible backend tools
```

Apps are therefore **not a separate transport**. They remain tools/resources on the same MCP connection, with additional metadata and renderer resources layered on top.

---

## 25.1 App architecture decision table

| Need | Prefer | Why |
| --- | --- | --- |
| One chart/table/dashboard returned by one tool | `@mcp.tool(app=True)` | Lowest ceremony; normal tool with a Prefab return |
| UI with several server-side backend actions | `FastMCPApp` | Stable backend identity, app/model visibility, composition-safe routing |
| Standard human confirmation UX | `Approval()` | Adds a reusable model-visible approval card tool |
| Fixed choices as buttons | `Choice()` | Removes ambiguous free-text selection UX |
| Structured form generated from a Pydantic model | `FormInput(model=...)` | Server-side schema validation and typed submission |
| Drag-and-drop files outside the LLM context window | `FileUpload()` | UI upload + model-visible list/read tools |
| Let the model synthesize a bespoke Prefab UI | `GenerativeUI()` | Runtime model-authored UI in a sandbox |
| Full custom frontend/framework | MCP Apps custom HTML / `AppConfig` | Maximum control over HTML/CSS/JS and renderer behavior |

Agent rule: start with the smallest app abstraction that satisfies the interaction. Do not build a custom renderer when a built-in provider or Prefab component tree expresses the workflow cleanly.

---

## 25.2 `app=True`: display-first Prefab tools

Representative stable pattern:

```python
from fastmcp import FastMCP
from prefab_ui.app import PrefabApp
from prefab_ui.components import Column, Heading, Text

mcp = FastMCP("Reports")

@mcp.tool(app=True)
def show_summary(project: str) -> PrefabApp:
    with Column(gap=4) as view:
        Heading(f"Summary — {project}")
        Text("Rendered as an MCP App rather than plain text only.")
    return PrefabApp(view=view)
```

Use `app=True` when:

* the tool already has a natural model-facing invocation;
* its primary result is visual or interactive;
* the UI can be represented by Prefab;
* backend tool topology is minimal.

Do not treat the rendered UI as the only contract. Tool `content` and `structuredContent` still matter for compatibility, logging, testing, and hosts that do not render Apps.

### Automatic Prefab recognition

FastMCP can recognize Prefab return annotations and supply the app metadata/renderer path automatically. For agent-authored production code, explicit `app=True` is often clearer because it makes the UI contract visible at the decorator rather than relying on type-driven inference.

---

## 25.3 `FastMCPApp`: server-connected application bundle

When an app needs backend tools, use `FastMCPApp` as a provider:

```python
from fastmcp import FastMCP, FastMCPApp
from prefab_ui.app import PrefabApp

contacts = FastMCPApp("Contacts")

@contacts.tool()
def save_contact(name: str, email: str) -> dict:
    return {"name": name, "email": email, "saved": True}

@contacts.ui()
def contacts_ui() -> PrefabApp:
    # Construct the Prefab UI and refer to backend functions/actions here.
    ...

mcp = FastMCP("Platform", providers=[contacts])
```

The important semantic split is:

```text
@app.ui()    -> model-visible entry tool by default
@app.tool()  -> app-visible backend tool by default
```

This avoids exposing every UI implementation helper to the model while still allowing the renderer to invoke those helpers. FastMCP tags and resolves backend tools by app identity so namespacing/composition does not accidentally break UI calls.

### Why string-based backend names are fragile

A plain `CallTool("save_contact")` string can become stale if a provider is namespaced or transformed. `FastMCPApp` lets FastMCP resolve backend identities late enough that the client-visible name can change while the app relationship remains intact.

---

## 25.4 `Approval`: human-in-the-loop UX, not a security boundary

```python
from fastmcp import FastMCP
from fastmcp.apps.approval import Approval

mcp = FastMCP("Operations")
mcp.add_provider(Approval())
```

The provider adds a model-visible approval tool that presents an Approve/Reject UI and returns the user's choice through the conversation.

Use it for:

* deployment confirmations;
* purchase/payment UX;
* bulk mutation confirmation;
* user-friendly review before a sensitive action.

Critical invariant:

```text
Approval UI = advisory interaction
server authorization / state transition = enforcement
```

A model can ignore an advisory card, a conversation can continue, and hosts differ in interaction semantics. If approval is a hard requirement, the mutation tool itself must verify an enforceable server-side approval token/state/authorization condition before changing anything.

Example policy shape:

```python
@mcp.tool
def delete_records(batch_id: str, approval_token: str) -> dict:
    verify_server_side_approval(batch_id, approval_token)
    return perform_delete(batch_id)
```

The UI can obtain/present the approval flow, but enforcement belongs inside trusted server logic.

---

## 25.5 `Choice`: bounded selection UX

```python
from fastmcp import FastMCP
from fastmcp.apps.choice import Choice

mcp = FastMCP("Planner")
mcp.add_provider(Choice(title="Choose an Option"))
```

`Choice` is useful when the correct interaction is a small closed set rather than arbitrary prose. Typical cases:

* deployment strategy;
* environment selection;
* report format;
* workflow branch;
* “pick one of these candidates.”

Like `Approval`, it is interaction UX, not policy enforcement. If only three enum values are valid for a later tool, that later tool should still validate the enum itself.

---

## 25.6 `FormInput`: Pydantic model -> validated interactive form

```python
from typing import Literal
from pydantic import BaseModel, Field
from fastmcp import FastMCP
from fastmcp.apps.form import FormInput

class IncidentReport(BaseModel):
    title: str = Field(description="Short incident title")
    severity: Literal["low", "medium", "high", "critical"]
    description: str = Field(description="What happened")

mcp = FastMCP("Incident Intake")
mcp.add_provider(FormInput(model=IncidentReport))
```

This is the preferred app primitive when:

* the interaction should produce a structured object;
* validation belongs on the server;
* free-text parsing would be brittle;
* the form can be generated from Pydantic/JSON Schema.

Design guidance:

1. Keep the Pydantic model narrow and user-facing.
2. Use `Field(...)` descriptions and constraints as the canonical form contract.
3. Validate again at the final business action boundary when the action has additional contextual constraints.
4. Do not put secrets into form defaults or renderer metadata.

---

## 25.7 `FileUpload`: data plane outside the model context window

```python
from fastmcp import FastMCP
from fastmcp.apps.file_upload import FileUpload

mcp = FastMCP("Document Tools")
mcp.add_provider(
    FileUpload(
        max_file_size=10 * 1024 * 1024,
        title="Upload documents",
    )
)
```

The built-in provider exposes an upload UI plus model-visible file-list/read capabilities. This is high leverage because binary/file ingress can happen through the app renderer rather than forcing the file bytes through the model context.

### Session-scope warning

Default file storage is session-scoped and in-memory. That aligns naturally with STDIO, SSE, and stateful HTTP sessions. It is **not sufficient for stateless HTTP** because consecutive requests may not share the same session identifier.

For stateless or horizontally scaled deployments:

```text
UI upload request
    -> durable object/file store keyed by authenticated principal or stable workflow id
model list/read request
    -> same durable store and same stable authorization scope
```

Never derive the stable key from a client-supplied filename or unchecked arbitrary string. Prefer authenticated subject/tenant IDs plus server-generated object IDs.

### File security checklist

* enforce a server-side maximum byte size;
* sanitize/replace filenames rather than trusting paths;
* reject path traversal and absolute paths;
* content-sniff or validate MIME where relevant;
* virus/malware scan when files enter higher-risk enterprise workflows;
* use authenticated scope keys in stateless deployments;
* apply retention/deletion policy;
* do not expose one user's uploads to another principal.

---

## 25.8 `GenerativeUI`: model-authored Prefab at runtime

```python
from fastmcp import FastMCP
from fastmcp.apps.generative import GenerativeUI

mcp = FastMCP("Prefab Studio")
mcp.add_provider(GenerativeUI())
```

The provider gives the model a UI-generation tool and a component-search capability. The model writes Prefab Python dynamically; execution/rendering occurs in sandboxed environments rather than running arbitrary generated Python directly in the host process.

Use Generative UI when the **layout itself is part of the reasoning problem**: exploratory dashboards, ad hoc visualization, contextual forms, or synthesized work surfaces.

Avoid it when:

* the UI must be pixel-stable and thoroughly pretested;
* model-generated code is unacceptable under the threat model;
* the workflow has strict accessibility/compliance review requirements that demand fixed UI artifacts;
* a simple static Prefab tool already solves the problem.

### Sandbox is necessary, not sufficient

Sandboxing generated UI code limits execution risk, but your backend tools still define what effects are possible. Apply normal auth, authorization, validation, and destructive-action policy to every backend tool. Never assume “the code ran in Pyodide” means the full application is safe.

---

## 25.9 Prefab version pinning

FastMCP intentionally does not make the app renderer dependency a permanently frozen implementation detail. The Prefab surface evolves quickly, so production deployments should pin **both** the FastMCP version and the Prefab UI version used in acceptance tests.

Example:

```toml
[project]
dependencies = [
  "fastmcp==3.4.7",
  "prefab-ui==<YOUR_TESTED_VERSION>",
]
```

Do not copy an arbitrary Prefab version from this reference: pin the version your project actually tests. The important rule is to avoid an unconstrained fresh resolution on production deploys.

---

## 25.10 Custom HTML / low-level MCP Apps

Use a custom renderer when Prefab cannot express the interface. The low-level pattern is conceptually:

```text
tool metadata -> resourceUri = ui://...
ui:// resource -> HTML/JS/CSS renderer
ToolResult.structured_content -> application data
iframe <-> host AppBridge -> tool calls/messages/state
```

Production custom-renderer concerns:

* CSP (`ResourceCSP`) and explicit resource origins;
* permissions (`ResourcePermissions`);
* no secrets embedded in HTML payloads;
* strict input validation on UI-originated tool calls;
* versioned frontend assets;
* browser isolation assumptions;
* compatibility testing in every target MCP host.

If the UI requires a mature React/Vue/Svelte application, keep the frontend build system explicit instead of hiding a large frontend bundle inside an improvised Python string.

---

## 25.11 App testing matrix

Test at four layers:

| Layer | Test |
| --- | --- |
| Tool contract | Entry tool appears with intended model visibility and schema |
| Structured result | `structuredContent` contains the expected app payload |
| Backend routing | App-visible backend tool resolves after namespace/provider transforms |
| Host rendering | Target host actually renders and can round-trip actions |

Additional cases:

```text
[ ] model-visible app entry exists
[ ] app-only backend tools are not leaked into model tool list
[ ] namespace transform does not break backend references
[ ] unauthorized backend call is rejected
[ ] renderer handles tool error state
[ ] file upload limit is enforced server-side
[ ] stateless deployment uses stable file scope
[ ] approval is not treated as security enforcement
[ ] Prefab version is pinned in the project lockfile
```

---

## 25.12 Anti-pattern inventory

* Treating `Approval` as cryptographic authorization.
* Exposing every app backend helper to the model.
* Hard-coding transformed tool names inside a composable app.
* Running user/model-authored Python in the FastMCP server process instead of a sandbox.
* Using default session-scoped `FileUpload` storage in stateless HTTP.
* Trusting uploaded filenames as filesystem paths.
* Letting an unconstrained `prefab-ui` version float in production.
* Assuming every MCP client renders Apps.
* Omitting plain-content/error behavior because “the UI will handle it.”
* Putting secrets into renderer metadata or `structuredContent`.

---

## 25.13 Agent checklist

```text
[ ] Choose app=True vs FastMCPApp vs built-in provider vs GenerativeUI vs custom HTML deliberately.
[ ] Keep app-only backend tools hidden from the model unless model access is intentional.
[ ] Treat Approval/Choice as UX, not enforcement.
[ ] Use Pydantic validation for FormInput contracts.
[ ] Give FileUpload a durable authenticated scope in stateless/scaled deployments.
[ ] Enforce file size/path/type policy server-side.
[ ] Apply auth/authorization to every effectful backend tool.
[ ] Pin prefab-ui alongside FastMCP for production Apps.
[ ] Test structuredContent plus actual host rendering.
[ ] Verify namespaces/transforms do not break app-backend routing.
```

### Sources

1. https://gofastmcp.com/apps/overview
2. https://gofastmcp.com/apps/architecture
3. https://gofastmcp.com/apps/prefab
4. https://gofastmcp.com/apps/fastmcp-app
5. https://gofastmcp.com/apps/providers/approval
6. https://gofastmcp.com/apps/providers/choice
7. https://gofastmcp.com/apps/providers/form
8. https://gofastmcp.com/apps/providers/file-upload
9. https://gofastmcp.com/apps/generative

# FastMCP Advanced — 26) OpenAPI and FastAPI integration
### OpenAPI and FastAPI integration

### 26.0 Scope and stable-version stance

FastMCP can generate MCP components from OpenAPI specifications and from FastAPI applications. In the v3 architecture, this is provider-driven: an `OpenAPIProvider` is a component source, and `FastMCP.from_openapi(...)` / `FastMCP.from_fastapi(...)` are convenience construction paths around that capability.

Use this integration as an **accelerator**, not as a claim that REST endpoint design and model-facing tool design are equivalent. A generated server can expose an API quickly, but large or implementation-shaped OpenAPI surfaces usually require curation, descriptions, exclusions, transforms, or a hand-designed facade before they become high-quality agent interfaces.

Version note: the stable target of this reference is FastMCP 3.4.7. The 4.0 prerelease changes FastMCP's internal HTTP stack to `httpx2`; v3.4.x projects should use the HTTP client type supported by their pinned 3.x environment and not copy v4-only imports into stable code. Section 36 covers that migration explicitly.

---

## 26.1 `FastMCP.from_openapi(...)`: bootstrap an MCP server from a spec

Stable conceptual pattern:

```python
import httpx
from fastmcp import FastMCP

api_client = httpx.AsyncClient(
    base_url="https://api.example.com",
    headers={"Authorization": "Bearer ..."},
)

spec = {...}  # parsed OpenAPI document

mcp = FastMCP.from_openapi(
    openapi_spec=spec,
    client=api_client,
    name="Example API",
)
```

The generated server is still a normal `FastMCP` server. You can add handwritten tools/resources/prompts, providers, transforms, middleware, auth, and visibility rules after generation.

### What generation has to do

```text
OpenAPI document
  -> parse paths/operations/parameters/request bodies/responses
  -> generate FastMCP components
  -> map operation inputs to MCP input schemas
  -> use configured HTTP client to call upstream API
  -> convert upstream result to MCP result
```

The upstream HTTP API and the MCP server are two distinct trust boundaries. Do not collapse their authentication or error models accidentally.

---

## 26.2 `OpenAPIProvider`: provider-first composition

When building larger systems, prefer thinking in provider terms:

```python
from fastmcp import FastMCP
from fastmcp.server.providers.openapi import OpenAPIProvider

provider = OpenAPIProvider(
    openapi_spec=spec,
    client=api_client,
)

mcp = FastMCP("Platform", providers=[provider])
```

Provider-first design is useful when:

* the generated API is only one component source among several;
* you want provider-local transforms;
* several OpenAPI sources are composed under namespaces;
* the provider lifecycle/configuration is managed independently;
* you want to wrap or reuse the source without copying components.

The public convenience constructor is excellent for a one-source server; `OpenAPIProvider` better exposes the v3 architecture for platform composition.

---

## 26.3 Default route mapping: endpoints become tools

FastMCP's current default is intentionally compatibility-oriented: OpenAPI operations become **tools** unless custom route maps say otherwise. This is a pragmatic choice because tool support is the broadest MCP client capability.

Do not infer semantic quality from that default. A read-only `GET /users/{id}` may be semantically resource-like, but exposing it as a tool can still be the better interoperability choice.

---

## 26.4 `RouteMap` and `MCPType`

Custom route mapping lets you decide what becomes a tool, resource, resource template, or excluded operation.

```python
from fastmcp import FastMCP
from fastmcp.server.providers.openapi import RouteMap, MCPType

route_maps = [
    RouteMap(
        pattern=r"^/admin/.*",
        mcp_type=MCPType.EXCLUDE,
    ),
    RouteMap(
        methods=["GET"],
        pattern=r"^/public/.*",
        mcp_type=MCPType.RESOURCE,
    ),
]

mcp = FastMCP.from_openapi(
    openapi_spec=spec,
    client=api_client,
    route_maps=route_maps,
)
```

Route-map design dimensions include:

* HTTP method;
* path regex;
* OpenAPI tags;
* target MCP component type;
* custom MCP tags;
* exclusion.

Rules are ordered. Treat rule order as part of the public contract because the first applicable mapping determines how the route is exposed.

---

## 26.5 Exclusion-first policy for large or sensitive APIs

For production-generated servers, default-deny is usually safer than mirroring every endpoint.

Recommended curation sequence:

```text
1. Exclude admin/internal/debug/bulk-destructive endpoints.
2. Exclude endpoints that expose secrets or implementation metadata.
3. Keep a narrow allowlist of useful model-facing operations.
4. Rewrite descriptions and parameter names for model clarity.
5. Namespace or tag the generated surface.
6. Add authorization to effectful components.
7. Add ToolSearch/Code Mode only after the catalog itself is policy-safe.
```

Search transforms reduce **discovery volume**; they do not make an unsafe hidden endpoint safe. Authorization/exclusion must be correct before search is layered on top.

---

## 26.6 Upstream authentication vs MCP authentication

Keep two identities separate:

```text
MCP caller identity
   -> FastMCP authentication/authorization
FastMCP service identity (or delegated caller credential)
   -> upstream API authentication
```

### Service-credential pattern

```python
api_client = httpx.AsyncClient(
    base_url=API_BASE,
    headers={"Authorization": f"Bearer {SERVICE_TOKEN}"},
)
```

All authorized MCP users call the upstream API as the server's service identity. This is simple but means upstream audit logs do not natively distinguish MCP callers.

### Delegated identity pattern

If upstream calls must act as the actual user, extract a validated FastMCP access token/principal and construct a tightly scoped per-request upstream credential. Do **not** blindly forward every inbound header or cookie.

Security rule:

```text
validated claim -> explicit upstream credential mapping
not
incoming HTTP headers -> copy all to upstream
```

This avoids credential confusion and accidental leakage across origins.

---

## 26.7 OpenAPI server variables and parameter encodings

Real specifications may use:

* server URL variables;
* path/query/header/cookie parameters;
* arrays and nested values;
* `deepObject` query encoding;
* multiple content types;
* request-body schemas;
* response schemas.

FastMCP 3.x continued to improve these edge cases. When the generated server is business-critical, add integration fixtures from the actual API for every nontrivial parameter style rather than assuming the OpenAPI schema conversion alone guarantees correct wire encoding.

Test the **HTTP request actually sent**, not only the MCP tool schema.

---

## 26.8 OpenAPI security posture

OpenAPI generation is a high-risk boundary because a model-facing parameter becomes part of an outbound HTTP request. The FastMCP project published security fixes in 2026 around OpenAPI-provider SSRF/path traversal behavior. Production policy should therefore include:

* pinning a patched current stable release (3.4.7 for this document);
* restricting base URLs/origins;
* preventing user-controlled scheme/host changes;
* validating path parameters and normalizing URL joining;
* blocking internal metadata/link-local destinations unless explicitly needed;
* keeping admin/internal routes excluded;
* constraining redirects and proxy behavior;
* testing hostile path/query inputs.

Do not expose an arbitrary “URL” parameter just because an upstream OpenAPI description contains one. Review generated schemas as security-sensitive code.

---

## 26.9 `FastMCP.from_fastapi(...)`

FastAPI conversion is a convenience path built on OpenAPI generation:

```python
from fastapi import FastAPI
from fastmcp import FastMCP

app = FastAPI()

@app.get("/products/{product_id}")
async def get_product(product_id: int):
    return {"id": product_id}

mcp = FastMCP.from_fastapi(app=app)
```

The result is a normal `FastMCP` server, so handwritten MCP components can be added after conversion.

Use `from_fastapi` when:

* you own an existing FastAPI service and need a fast MCP bootstrap;
* the OpenAPI document is already a good description of endpoint schemas;
* you will curate the generated surface.

Do not use it as a reason to expose every operational endpoint to the model.

---

## 26.10 FastAPI conversion vs mounting FastMCP into FastAPI

These are opposite directions:

```text
FastMCP.from_fastapi(app)
    FastAPI/OpenAPI -> generated MCP components

fastapi_app.mount(..., mcp.http_app(...))
    FastMCP server -> mounted ASGI application inside FastAPI
```

They solve different problems and can even coexist.

### Existing API -> MCP

Use `from_fastapi`.

### Existing web service needs an MCP endpoint

Use `mcp.http_app()` and mount it into FastAPI; forward/merge FastMCP lifespan correctly (Section 19/20).

Agent rule: never substitute one for the other because both names mention FastAPI.

---

## 26.11 Generated schemas vs model-facing schemas

An OpenAPI operation can be syntactically correct yet poor for model use. Typical problems:

* implementation-shaped names (`post_v1_accounts_id_reconcile`);
* too many irrelevant parameters;
* undocumented enums;
* nested request objects with poor field descriptions;
* identical-looking endpoints that differ only subtly;
* administrative routes mixed with end-user operations;
* dozens/hundreds of tools competing for model attention.

Remedies:

```text
RouteMap exclusions
  + Namespace
  + ToolTransform (rename / description / argument shaping)
  + tags and visibility
  + ToolSearch / Code Mode for very large safe catalogs
  + handwritten facade tools for high-value workflows
```

The strongest pattern is often to treat generated components as a **low-level provider** and publish a curated higher-level surface above them.

---

## 26.12 Workflow facade pattern

Instead of exposing five REST endpoints as five unrelated tools:

```text
POST /draft
POST /validate
POST /approve
POST /commit
GET  /status
```

publish a deliberate workflow tool:

```python
@mcp.tool
def submit_order(order: OrderRequest) -> OrderResult:
    """Validate, submit, and return the accepted order state."""
    ...
```

Internally, the tool can use the same HTTP client/API. This yields:

* fewer model decisions;
* stronger transaction semantics;
* better validation;
* easier authorization;
* better error handling;
* a stable MCP contract even if REST endpoints change.

Generated OpenAPI tools remain useful as lower-level maintenance/debug capabilities if properly restricted.

---

## 26.13 Testing generated servers

Test both layers.

### MCP contract tests

```python
from fastmcp import Client

async with Client(mcp) as client:
    tools = await client.list_tools()
    assert all(t.name not in {"internal_reset", "admin_delete"} for t in tools)
```

### Upstream request tests

Mock or capture the HTTP request and assert:

```text
[ ] method correct
[ ] path variables encoded correctly
[ ] query encoding correct
[ ] headers contain only intended auth/context
[ ] body matches upstream schema
[ ] redirects/origins constrained
[ ] HTTP error mapped intentionally
```

### Security tests

Use hostile inputs for:

```text
../ traversal
absolute URL substitution
localhost / 127.0.0.1
link-local metadata addresses
encoded slashes / percent encoding
unexpected redirect target
oversized nested query parameters
```

---

## 26.14 Anti-pattern inventory

* Mirroring a 500-endpoint REST API into 500 model-visible tools and calling it finished.
* Forwarding every inbound Authorization/Cookie/header to the upstream API.
* Allowing tool arguments to replace the upstream host/scheme.
* Forgetting to exclude admin/internal operations.
* Treating ToolSearch as authorization.
* Confusing `from_fastapi()` with ASGI mounting.
* Relying only on MCP schema tests without checking emitted HTTP requests.
* Letting OpenAPI operation IDs become permanent public names without curation.
* Upgrading to v4 `httpx2` examples inside a project pinned to 3.4.x.
* Ignoring published provider security fixes and running a stale version.

---

## 26.15 Agent checklist

```text
[ ] Decide whether generation is the final public surface or only a provider/bootstrap layer.
[ ] Use a client compatible with the pinned FastMCP major version.
[ ] Exclude internal/admin/destructive routes first.
[ ] Review RouteMap order.
[ ] Separate MCP caller auth from upstream API auth.
[ ] Never blindly forward arbitrary inbound headers.
[ ] Pin to a patched stable FastMCP release.
[ ] Test SSRF/path-traversal/redirect cases.
[ ] Curate names/descriptions/argument schemas for model use.
[ ] Use ToolSearch/Code Mode only after the catalog is safe.
[ ] Test actual outbound HTTP requests.
[ ] Prefer workflow facade tools for multi-endpoint business actions.
```

### Sources

1. https://gofastmcp.com/integrations/openapi
2. https://gofastmcp.com/integrations/fastapi
3. https://gofastmcp.com/servers/providers/overview
4. https://gofastmcp.com/servers/providers/custom
5. https://github.com/PrefectHQ/fastmcp/security/advisories/GHSA-vv7q-7jx5-f767
6. https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2

# FastMCP Advanced — 27) Project configuration, settings, and portable deployment contracts
### Project configuration, settings, and portable deployment contracts

### 27.0 `fastmcp.json` as the canonical project contract

`fastmcp.json` is the **canonical and preferred** project-configuration format for FastMCP. The docs state that it provides a single portable source of truth for server settings, dependencies, and deployment options, replacing ad hoc command-line arguments and shell scripts. It is explicitly designed to be shareable across environments and teams. ([FastMCP][1])

The configuration model is intentionally tri-partite:

* **source** = where server code lives
* **environment** = what runtime/build environment the server requires
* **deployment** = how the server should run at execution time. ([FastMCP][1])

Minimal canonical shape:

```json id="bmo8q1"
{
  "$schema": "https://gofastmcp.com/public/schemas/fastmcp.json/v1.json",
  "source": {
    "path": "server.py",
    "entrypoint": "mcp"
  },
  "environment": {
    "type": "uv",
    "python": ">=3.10",
    "dependencies": ["pandas", "numpy"]
  },
  "deployment": {
    "transport": "stdio",
    "log_level": "INFO"
  }
}
```

Only `source` is required; `environment` and `deployment` are optional. ([FastMCP][1])

### 27.1 Why `fastmcp.json` should be the default project artifact

Use `fastmcp.json` whenever you want reproducible deployments, explicit dependency declarations, portable run/install behavior, and a stable handoff artifact between development, CI/CD, and end-user installation workflows. The docs explicitly tie the format to reproducible environments and to seamless use with `fastmcp run`, `install`, `inspect`, and related commands. ([FastMCP][1])

It is also the right format for local-to-production continuity because the same file can drive direct execution, installation into local MCP host clients, JSON config generation for unsupported clients, and prebuilt environment workflows. That makes it more than “configuration”; it is the deployment contract for the server. ([FastMCP][1])

### 27.2 `source`: where the server code lives

The `source` section is required. Today, the documented source type is `filesystem` (default), pointing at a local Python file with a FastMCP server instance or a zero-argument factory function. If `entrypoint` is omitted, FastMCP searches for common names such as `mcp`, `server`, or `app`. File paths are resolved relative to the configuration file’s location. The docs also note future planned source types like `git` and `cloud`, but those are not the currently supported contract. ([FastMCP][1])

Representative shape:

```json id="lh7yrm"
{
  "source": {
    "type": "filesystem",
    "path": "src/server.py",
    "entrypoint": "mcp"
  }
}
```

Use an explicit `entrypoint` whenever the module exports more than one plausible server object or when you are using a factory function; rely on name inference only for simple one-server modules. That recommendation follows directly from the documented entrypoint-resolution behavior. ([FastMCP][1])

### 27.3 `environment`: what must exist to run the server

The `environment` section is the build-time environment contract. Current docs say FastMCP supports `uv` environments here, with fields for `python`, `dependencies`, `requirements`, `project`, and `editable`. When any UV-environment field is specified, FastMCP creates an isolated environment before running the server. ([FastMCP][1])

Documented UV fields:

* `python`: exact version or version constraint such as `"3.12"` or `">=3.10,<3.13"`
* `dependencies`: PEP 508 package strings
* `requirements`: requirements file path, resolved relative to the config file
* `project`: directory containing `pyproject.toml`
* `editable`: list of packages/directories to install in editable mode. ([FastMCP][1])

Representative shape:

```json id="acifjg"
{
  "environment": {
    "type": "uv",
    "python": ">=3.10",
    "dependencies": ["pandas>=2.0", "httpx"],
    "editable": ["."]
  }
}
```

This section is the correct place for dependency and interpreter requirements. Do not treat `fastmcp install` flags as the long-term source of truth when a project already has stable requirements; encode them here and let CLI operations consume the file. That is the model the docs promote. ([FastMCP][1])

### 27.4 `deployment`: how the server runs

The `deployment` section is the run-time execution contract. The docs define fields for `transport`, `host`, `port`, `path`, `log_level`, `env`, `cwd`, and `args`. Current documented transport values are `stdio`, `http`, and `sse`, with defaults of `stdio`, `127.0.0.1`, port `3000`, and path `/mcp/` in the config schema page. ([FastMCP][1])

Representative shape:

```json id="pnc6wg"
{
  "deployment": {
    "transport": "http",
    "host": "0.0.0.0",
    "port": 3000,
    "path": "/api/mcp/",
    "log_level": "INFO",
    "env": {
      "ENV": "production",
      "API_BASE_URL": "https://api.example.com"
    },
    "cwd": "/app",
    "args": ["--workers", "4"]
  }
}
```

`deployment.env` supports runtime interpolation of environment variables using `${VAR_NAME}` syntax; missing variables are left intact rather than failing expansion. The docs explicitly call out this pattern as useful for multi-environment deployments and for keeping sensitive values out of checked-in config files. ([FastMCP][1])

This section should be treated as **runtime behavior**, not package management. Build/install belongs in `environment`; process invocation shape belongs in `deployment`. The docs make that separation explicitly. ([FastMCP][1])

### 27.5 JSON-schema support for `fastmcp.json`

FastMCP publishes JSON Schemas for `fastmcp.json`, and the docs recommend adding `$schema` for IDE autocomplete and validation. Two schema URLs are documented: a version-specific URL and a rolling `latest.json` URL. ([FastMCP][1])

```json id="3m8jqk"
{
  "$schema": "https://gofastmcp.com/public/schemas/fastmcp.json/v1.json",
  "source": {
    "path": "server.py",
    "entrypoint": "mcp"
  }
}
```

Use the version-specific schema in production repositories so validation stays stable across FastMCP upgrades; use `latest.json` only when you explicitly want the schema to track the newest format automatically. The docs list both URLs; the stability recommendation is the conservative interpretation of that dual-publishing model. ([FastMCP][1])

### 27.6 CLI overrides and precedence

CLI arguments override `fastmcp.json` values. The project-configuration docs show this explicitly for transport, port, and additional dependencies, and the utility layer exposes a `load_and_merge_config(...)` helper to merge config with CLI overrides across commands like `run`, `inspect`, and `dev`. ([FastMCP][1])

Representative examples from the documented precedence model:

```bash id="mxgsyn"
fastmcp run fastmcp.json --port 8080
fastmcp run fastmcp.json --transport http
fastmcp run fastmcp.json --with requests --with httpx
```

This precedence is what makes `fastmcp.json` viable as the canonical baseline while still allowing environment- or operator-specific overrides without editing the file. Treat the file as default truth and the CLI as ephemeral override. ([FastMCP][1])

### 27.7 Auto-detection and file naming

FastMCP auto-detects a file specifically named `fastmcp.json` in the current directory. Files with the same structure but different names are **not** auto-detected and must be passed explicitly. The docs show `fastmcp run` with no arguments relying on this exact filename rule. ([FastMCP][1])

```bash id="i0qko0"
cd my-project
fastmcp run              # auto-detects ./fastmcp.json
fastmcp run prod.fastmcp.json
```

That makes `fastmcp.json` not just a schema name but an operational convention. If you want zero-argument CLI ergonomics for a project, use that exact filename. ([FastMCP][1])

---

## 27.8 `fastmcp run`: operational entrypoint

`fastmcp run` starts a server or bridges to one. The CLI running docs say it accepts a Python file, explicit file/object entrypoint, factory function, remote URL, `fastmcp.json`, standard MCP config JSON, or no argument with `fastmcp.json` auto-detection. The lower-level CLI reference additionally documents module mode (`-m my_module`) and `--` passthrough of server arguments. ([FastMCP][2])

Representative target forms:

```bash id="kw4mbf"
fastmcp run server.py
fastmcp run server.py:my_server
fastmcp run server.py:create_server
fastmcp run https://example.com/mcp
fastmcp run fastmcp.json
fastmcp run mcp.json
fastmcp run -m my_module
fastmcp run server.py -- --config config.json --debug
```

Two operational details matter a lot:

1. By default, `fastmcp run` uses `stdio`; use `--transport http` or `--transport sse` to expose a network service. ([FastMCP][2])
2. `fastmcp run` **ignores** `if __name__ == "__main__"` blocks. The docs explicitly say any setup logic there will not execute; if startup logic must run, expose a factory function and target that instead. ([FastMCP][2])

This makes factory functions the correct CLI entrypoint when initialization side effects, configuration bootstrapping, or async setup must happen before the server is returned. ([FastMCP][2])

### 27.8.1 Dependency-management behavior in `run`

The running docs say `fastmcp run` uses the current Python environment directly by default, but switches to `uv run` subprocess execution when flags like `--python`, `--with`, `--project`, or `--with-requirements` are present. In the `fastmcp.json` workflow, environment config can also trigger isolated-environment setup. The same docs and project-config docs document `--skip-env`, `--skip-source`, and `--project` for advanced environment control. ([FastMCP][2])

Use `--skip-env` when you already have a correct environment (activated venv, container image, prebuilt CI environment, or uv-managed project). Use `fastmcp project prepare` plus `--project` when you want install-once, run-many deployment behavior. ([FastMCP][1])

---

## 27.9 `fastmcp project prepare`: prebuilding reusable environments

Although not one of the user’s explicitly requested commands, `project prepare` belongs in the same operational layer because the `fastmcp.json` docs present it as the standard prebuild workflow. It creates a persistent uv project from `fastmcp.json`, installing dependencies ahead of time so later `fastmcp run ... --project <dir>` is fast and reproducible. The docs say the prepared directory contains `pyproject.toml`, `.venv`, and `uv.lock`. ([FastMCP][2])

```bash id="dn0jv8"
fastmcp project prepare fastmcp.json --output-dir ./env
fastmcp run fastmcp.json --project ./env
```

Use this in CI/CD, Docker image builds, and production deployments where dependency resolution at startup is undesirable. That recommendation follows directly from the docs’ “slow build, fast run” positioning. ([FastMCP][2])

---

## 27.10 `fastmcp install`: local-host installation and configuration generation

`fastmcp install` registers a local server with an MCP host application so the host can launch it automatically. The install docs say it is designed around local **stdio** servers and that each client runs servers in its own isolated environment, so dependencies must be declared explicitly rather than assumed from the local machine. `uv` must be available in `PATH`, and the docs call this out specifically for Claude Desktop and Cursor. ([FastMCP][3])

Supported install targets documented by FastMCP are:

* `claude-code`
* `claude-desktop`
* `cursor`
* `gemini-cli`
* `goose`
* `mcp-json`
* `stdio`. ([FastMCP][3])

Examples:

```bash id="3bj6gu"
fastmcp install claude-desktop server.py
fastmcp install claude-code server.py --with pandas --with matplotlib
fastmcp install cursor server.py -e .
fastmcp install claude-desktop fastmcp.json
```

The install surface also supports `--server-name`, `--with-editable`/`-e`, `--with`, `--env`, `--env-file`, `--python`, `--project`, `--with-requirements`, and (for `claude-desktop`) `--config-path`. Those flags are the correct way to declare isolated-host runtime dependencies and environment variables at install time when you are not using `fastmcp.json`. ([FastMCP][3])

The docs explicitly say `fastmcp install` is intended for **local** stdio servers. For remote HTTP servers, use the host client’s native configuration path instead; FastMCP’s install value is simplifying local `uv`/dependency/env setup. ([FastMCP][3])

### 27.10.1 `mcp-json` target

The `mcp-json` install target emits standard MCP configuration JSON instead of installing into a specific supported client. The install docs say this is useful for unsupported clients, CI/CD, or sharing portable server configs, and the integrations page defines the standard format as a `mcpServers` object with `command`, `args`, and optional `env`. ([FastMCP][3])

Representative output shape:

```json id="br8wu9"
{
  "server-name": {
    "command": "uv",
    "args": ["run", "--with", "fastmcp", "fastmcp", "run", "/path/to/server.py"],
    "env": {
      "API_KEY": "value"
    }
  }
}
```

The docs also note `--copy` to send generated JSON to the clipboard instead of stdout. Use `mcp-json` when FastMCP does not have a first-class installer for the target client or when you need a raw config artifact for external systems. ([FastMCP][3])

### 27.10.2 `stdio` target

The `stdio` install target prints the exact shell command an MCP host would use to launch the server over standard I/O. The install docs show it emitting a `uv run ... fastmcp run ...` command, and when the source is `fastmcp.json`, documented dependencies from the config are automatically included. `--copy` also works here. ([FastMCP][3])

```bash id="x1qv7t"
fastmcp install stdio server.py
# -> uv run --with fastmcp fastmcp run /absolute/path/to/server.py

fastmcp install stdio fastmcp.json
# -> uv run --with fastmcp --with pillow --with 'qrcode[pil]>=8.0' fastmcp run /path/to/server.py
```

Use this target when you need the launch command for another wrapper, host, shell script, or deployment system, but not a client-specific install side effect. ([FastMCP][3])

---

## 27.11 `fastmcp inspect`: summary and machine-readable introspection

`fastmcp inspect` loads a server and reports its tools, resources, prompts, version, and metadata. By default it prints a human-readable summary; with `--format`, it emits JSON. The docs define two JSON output modes:

* `--format fastmcp`: full FastMCP metadata, `snake_case`, includes tool tags, enabled status, output schemas, annotations, and custom metadata
* `--format mcp`: protocol view, `camelCase`, only standard MCP fields. ([FastMCP][4])

```bash id="hdydub"
fastmcp inspect server.py
fastmcp inspect server.py --format fastmcp
fastmcp inspect server.py --format mcp -o manifest.json
fastmcp inspect fastmcp.json
```

This split is extremely useful operationally. Use `fastmcp` format for internal debugging and full-framework introspection; use `mcp` format to verify exactly what external MCP clients will see. The docs explicitly present those as the two intended use cases. ([FastMCP][4])

The command supports the same local entrypoints as `fastmcp run`: inferred instances, explicit object selection, factory functions, and `fastmcp.json` configs. ([FastMCP][4])

---

## 27.12 `fastmcp generate-cli`: generated typed CLIs from tool schemas

`fastmcp generate-cli` is a v3 CLI feature that turns any server into a standalone typed Python CLI. The docs say it connects to a server, reads tool schemas, and writes a Python script where every tool becomes a real subcommand with typed flags, help text, and tab completion. It uses tool JSON Schema to derive the CLI contract. ([FastMCP][5])

Basic generation:

```bash id="4e5d6c"
fastmcp generate-cli weather
fastmcp generate-cli http://localhost:8000/mcp
fastmcp generate-cli server.py my_weather_cli.py
fastmcp generate-cli weather -f
```

The output path defaults to `cli.py`; `-f` overwrites an existing file. Generated scripts also include generic MCP operations—`list-tools`, `list-resources`, `read-resource`, `list-prompts`, and `get-prompt`—in addition to tool-specific subcommands. The docs emphasize that these generic operations reflect the server’s current state even if the server’s catalog changes after generation. ([FastMCP][5])

### 27.12.1 Typed parameter mapping

The docs define the schema-to-CLI mapping:

* simple JSON-schema types (`string`, `integer`, `number`, `boolean`) become typed flags
* arrays of simple types become repeatable flags
* complex types (objects, nested arrays, unions) become JSON-string arguments, with full schema shown in `--help`. ([FastMCP][5])

That makes the generated CLI notably stronger than generic `fastmcp call`. It is a **server-specific CLI surface** rather than a universal MCP inspector. Use it when the same server will be called repeatedly by humans or shell-capable agents. ([FastMCP][5])

### 27.12.2 Generated `SKILL.md`

Alongside the script, `generate-cli` writes a `SKILL.md` file. The docs define it as a Claude Code agent skill describing every tool’s exact invocation syntax, parameter flags, types, and descriptions, so an agent can use the CLI immediately without probing `--help`. `--no-skill` disables this generation. ([FastMCP][5])

```bash id="jgv9hl"
fastmcp generate-cli weather --no-skill
```

This is particularly valuable for agentic workflows because the generated CLI plus `SKILL.md` becomes a compact operational bundle: executable client + machine-readable usage manual. ([FastMCP][5])

### 27.12.3 Generated CLI architecture

The generated script is a **client**, not a packaged server wrapper. The docs say it reconnects to the target server on every invocation and stores the resolved transport in a `CLIENT_SPEC` variable at the top of the file. For generated scripts from local servers, that may be a baked-in `StdioTransport`; for remote targets, it may be a URL string. The docs also note that the script requires `fastmcp` as a dependency and can be run with `uv run --with fastmcp ...` when outside an existing FastMCP environment. ([FastMCP][5])

That makes generated CLIs easy to retarget: change `CLIENT_SPEC` to move from dev to prod, or from local subprocess to remote HTTP, without regenerating the whole wrapper. ([FastMCP][5])

---

## 27.13 `fastmcp.json` vs standard MCP JSON

These are different artifacts and should not be conflated.

* `fastmcp.json` is FastMCP’s **project/deployment contract**: source, environment, deployment, CLI auto-detection, and prebuild workflows. ([FastMCP][1])
* standard MCP JSON (`mcpServers` with `command` / `args` / `env`) is the **client-install/launch contract** understood by host applications like Claude Desktop, Cursor, and VS Code. ([FastMCP][6])

Use `fastmcp.json` inside your project as the canonical source of truth. Generate or install standard MCP JSON only at the boundary where an external MCP host needs to know how to launch the server. That is the architecture implied by the docs’ separation of these two formats. ([FastMCP][1])

---

## 27.14 Best-practice operational guidance

Use `fastmcp.json` as the default checked-in project artifact. Put dependency and runtime defaults there, then layer CLI overrides for one-off tests or environment-specific adjustments. ([FastMCP][1])

Prefer `fastmcp run fastmcp.json` or zero-argument `fastmcp run` in project directories over hand-maintained shell scripts, because the config file centralizes source lookup, environment setup, and deployment behavior. ([FastMCP][1])

Use `fastmcp project prepare` plus `fastmcp run ... --project ...` for production pipelines, container builds, and CI/CD when cold-start dependency resolution is undesirable. ([FastMCP][1])

Use `fastmcp install` for local stdio-host integration into Claude Code, Claude Desktop, Cursor, Gemini CLI, or Goose; use `mcp-json` or native client config for unsupported clients or automation; use `stdio` target when you want the exact launch command instead of installation side effects. ([FastMCP][3])

Use `fastmcp inspect --format fastmcp` for full-framework introspection and `--format mcp` for protocol-surface verification. Use `fastmcp generate-cli` when repeated operator/agent use justifies a server-specific typed CLI and accompanying `SKILL.md`. ([FastMCP][4])


[1]: https://gofastmcp.com/deployment/server-configuration "Project Configuration - FastMCP"
[2]: https://gofastmcp.com/cli/running "Running Servers - FastMCP"
[3]: https://gofastmcp.com/cli/install-mcp "Install MCP Servers - FastMCP"
[4]: https://gofastmcp.com/cli/inspecting "Inspecting Servers - FastMCP"
[5]: https://gofastmcp.com/cli/generate-cli "Generate CLI - FastMCP"
[6]: https://gofastmcp.com/integrations/mcp-json-configuration "MCP JSON Configuration  FastMCP - FastMCP"

# FastMCP Advanced — 28) CLI and developer workflows
### CLI and developer workflows

### 28.0 CLI mental model

The `fastmcp` CLI is not only a server launcher. In the stable v3 line it covers four workflow families:

```text
build/run        -> run, dev
inspect/test     -> inspect, list, call
package/install  -> install, generate-cli
discover/manage  -> discover, version/auth helpers
```

Use these commands to keep the MCP protocol surface observable outside your Python process. The CLI is especially valuable for LLM coding agents because it gives deterministic shell-level checks of what the server actually publishes.

---

## 28.1 Command map

| Command | Primary use |
| --- | --- |
| `fastmcp version` | Verify installed FastMCP/MCP environment |
| `fastmcp run ...` | Run a server target over chosen transport |
| `fastmcp dev ...` | Developer tooling, including Inspector/Apps preview |
| `fastmcp inspect ...` | Load local server and emit human or JSON manifest |
| `fastmcp list ...` | List remote/local MCP components through the client path |
| `fastmcp call ...` | Invoke a tool, read a resource, or render a prompt |
| `fastmcp discover` | Find servers configured in supported local MCP hosts |
| `fastmcp generate-cli ...` | Generate a typed standalone client CLI for one server |
| `fastmcp install ...` | Register a server into supported MCP client applications |

Exact options can evolve; use `fastmcp <command> --help` against the pinned installed version as the call-signature source.

---

## 28.2 `fastmcp run`: reproducible local/server launch

```bash
fastmcp run server.py
fastmcp run server.py:mcp
fastmcp run fastmcp.json
fastmcp run server.py --transport http
```

Use explicit entrypoints when a module contains multiple server-like objects or factories. Prefer `fastmcp.json` for projects with nontrivial environment/deployment configuration because it becomes a portable execution contract rather than a long shell command.

### Environment isolation

FastMCP CLI workflows integrate tightly with `uv` so a server can run with declared Python/dependency requirements. Treat CLI dependency flags as a development convenience; promote durable dependency requirements into `pyproject.toml`, lockfiles, and/or `fastmcp.json` for reproducible CI/production.

---

## 28.3 Development preview

Two high-value dev loops are:

```bash
# Protocol-oriented development / Inspector workflow
fastmcp dev server.py

# Apps browser preview / host simulation
fastmcp dev apps server.py
```

The Apps preview lets you exercise renderer behavior without first configuring a full external MCP host. Still validate against the actual target host before release because host implementation/capability differences remain real.

---

## 28.4 `fastmcp inspect`: definition-time manifest

```bash
fastmcp inspect server.py
fastmcp inspect server.py --format fastmcp
fastmcp inspect server.py --format mcp -o mcp-manifest.json
```

Two views matter:

* **FastMCP format** — framework metadata such as tags, versions, output schemas, visibility-related data, and FastMCP-specific fields.
* **MCP format** — protocol-facing representation clients actually receive.

Use the MCP-format manifest as a compatibility artifact. Use the FastMCP format for debugging framework-specific publication state.

Important: `inspect` is a **local loading** workflow. It is not a remote URL introspector. Use the client-facing `list`/`call` commands for live remote servers.

---

## 28.5 `fastmcp list`: live catalog inspection

Representative usage:

```bash
fastmcp list server.py
fastmcp list https://example.com/mcp
fastmcp list my-configured-server
```

Use `list` when you want the view after transport, initialization, auth, provider aggregation, transforms, visibility, and protocol listing behavior—not merely the Python definition.

This distinction is crucial in debugging:

```text
inspect says tool exists
but live list does not
    -> investigate auth, visibility, transport, session transform, provider init
```

---

## 28.6 `fastmcp call`: protocol smoke test

```bash
fastmcp call server.py add a=2 b=3
fastmcp call https://example.com/mcp get_weather city=London
fastmcp call server.py resource://config
fastmcp call server.py summarize --prompt topic=weather
```

Useful options include raw JSON output, explicit transport, auth, timeout, input JSON, and prompt mode.

### JSON argument rule

Simple values can often be supplied as `key=value`; complex nested arguments are more robust through `--input-json`.

```bash
fastmcp call server.py create_user \
  --input-json '{"name":"Ada","roles":["admin","analyst"]}'
```

Use raw JSON output in CI so assertions operate on structure rather than colored human text.

---

## 28.7 Interactive elicitation through the CLI

For v3-era server-initiated elicitation, the CLI client can prompt in the terminal when a tool requests additional user input. This makes `fastmcp call` useful for exercising interactive tool paths without an LLM host.

Do not assume that because CLI elicitation works, every target client supports the same interaction. Client capability checks still matter.

FastMCP 4 changes the modern-protocol elicitation model; see Section 36 rather than applying v4 interaction semantics to this stable v3 reference.

---

## 28.8 `fastmcp discover`: find configured local servers

```bash
fastmcp discover
fastmcp discover --source claude-code
```

The CLI can inspect common host configuration locations such as Claude Desktop, Claude Code, Cursor, Gemini CLI, Goose, and project MCP JSON configuration.

Use this for:

* inventorying local development MCP servers;
* finding the command/config a host is actually launching;
* diagnosing “works manually, fails in editor” mismatches;
* migration audits across hosts.

Do not treat discovery output as a security inventory of remote infrastructure; it is a local configuration-discovery feature.

---

## 28.9 `fastmcp install`: host configuration generation

Representative commands:

```bash
fastmcp install claude-desktop server.py
fastmcp install claude-code server.py --with pandas
fastmcp install cursor server.py --with-editable .
fastmcp install mcp-json server.py
fastmcp install stdio server.py
```

Supported clients/options can vary by FastMCP version. The important architecture is that `install` translates a FastMCP server target plus environment requirements into the configuration format or registration mechanism expected by the target MCP host.

### Isolation rule

Desktop/editor hosts frequently launch STDIO servers in an isolated environment. Never assume the host inherits the Python virtual environment from the shell where you developed the server. Declare dependencies explicitly.

---

## 28.10 `fastmcp generate-cli`: freeze server schemas into a typed operator CLI

```bash
fastmcp generate-cli weather
fastmcp generate-cli http://localhost:8000/mcp weather_cli.py
```

The generator connects to the server, reads tool schemas, and emits a Python CLI where tool operations become typed subcommands/flags. It also includes generic list/resource/prompt operations so the script remains useful as server state evolves.

The generator also emits an agent-oriented `SKILL.md` by default, documenting the exact invocation syntax. This is particularly useful when a coding agent should interact with a service through a narrow generated command surface instead of constructing MCP calls ad hoc.

### Drift warning

Generated typed tool subcommands reflect the schema at generation time. If the server contract changes, regenerate and review the CLI. Generic operations still query current server state, but a generated tool subcommand is a code artifact that can go stale.

---

## 28.11 CLI in CI/CD

Recommended pipeline:

```bash
# 1. import / server construction smoke check
fastmcp inspect server.py --format mcp -o current-mcp.json

# 2. compare manifest / fingerprint baseline
python scripts/check_contract.py current-mcp.json

# 3. launch test server (or use in-memory tests separately)
# 4. execute protocol smoke calls
fastmcp call server.py health --json

# 5. package/build/deploy
```

For remote staging:

```bash
fastmcp list https://staging.example.com/mcp
fastmcp call https://staging.example.com/mcp health --json
```

Add auth options/credentials through secure CI secret injection rather than command history or committed shell scripts.

---

## 28.12 Debugging decision tree

```text
Server won't import
  -> python import / fastmcp inspect

Server imports but host can't launch STDIO
  -> fastmcp run + inspect host config + dependency/env isolation

Remote HTTP connection fails
  -> fastmcp list URL + auth + proxy/Host/Origin/TLS checks

Tool missing
  -> inspect local -> list live -> visibility/auth/provider/transform diff

Tool schema wrong
  -> inspect --format mcp + tool fingerprint / type annotations

Tool executes wrong
  -> fastmcp call with --json + server logs/traces

App doesn't render
  -> fastmcp dev apps -> target host capability test
```

---

## 28.13 Shell/STDIO logging invariant

STDIO transport uses standard input/output as the protocol stream. Do not print arbitrary debug text to stdout in an STDIO server.

Use:

* Python logging to stderr;
* FastMCP `Context` logging to the client when appropriate;
* structured telemetry;
* explicit debug files/traces.

A stray `print()` can corrupt the protocol stream and create misleading client failures.

---

## 28.14 Anti-pattern inventory

* Running `python server.py` manually in one environment and assuming Claude/Cursor launches the same environment.
* Using only `inspect` to diagnose a live authenticated remote server.
* Parsing human CLI text in CI when JSON output exists.
* Committing bearer tokens in install commands/config files.
* Treating a generated CLI as permanently synchronized with a changing server.
* Printing logs to stdout in STDIO mode.
* Using `fastmcp dev` behavior as proof of production host compatibility.
* Keeping a 20-flag `fastmcp run` shell command instead of a portable config artifact.

---

## 28.15 Agent checklist

```text
[ ] Use fastmcp version to record the actual runtime version.
[ ] Use inspect for local definition manifests.
[ ] Use list/call for live protocol behavior.
[ ] Prefer JSON output for automation.
[ ] Test both STDIO and HTTP when both are deployment targets.
[ ] Declare dependencies for host-installed STDIO servers.
[ ] Use fastmcp.json for portable nontrivial server configuration.
[ ] Regenerate typed CLIs after schema changes.
[ ] Keep credentials out of shell history/repository files.
[ ] Never write arbitrary stdout logs in STDIO mode.
```

### Sources

1. https://gofastmcp.com/cli/overview
2. https://gofastmcp.com/cli/running
3. https://gofastmcp.com/cli/client
4. https://gofastmcp.com/cli/inspecting
5. https://gofastmcp.com/cli/generate-cli
6. https://gofastmcp.com/cli/install-mcp
7. https://gofastmcp.com/deployment/server-configuration

# FastMCP Advanced — 29) Observability, inspection, telemetry, and operational diagnostics
### Observability, inspection, telemetry, and operational diagnostics

### 29.0 Operational framing

This section is the operational closure of the reference set: how to observe a running FastMCP system, verify the server surface before clients see it, test it without accidental transport or environment drift, and harden the deployment with explicit production defaults. FastMCP’s own docs cover these as separate surfaces—native OpenTelemetry instrumentation, `fastmcp inspect`, a dedicated testing guide, transport guidance, app versioning cautions, response-caching caveats, task-backend recommendations, and tool-catalog search transforms. Collapsing them into one ops section is the right final-layer mental model. ([FastMCP][1])

### 29.1 Built-in OpenTelemetry instrumentation

FastMCP includes native OpenTelemetry instrumentation by default. The telemetry docs say instrumentation is always active, creates spans for tool, prompt, resource, and resource-template operations, adds no overhead when no OpenTelemetry SDK is configured, and works with any OTLP-compatible backend because FastMCP uses the OpenTelemetry API and expects you to bring your own SDK/exporter/sampling policy. ([FastMCP][1])

The documented “fastest path” to exported traces is auto-instrumentation with `opentelemetry-instrument`: install `opentelemetry-distro` plus an OTLP exporter, run `opentelemetry-bootstrap -a install`, then launch the server under `opentelemetry-instrument fastmcp run server.py`, optionally setting `OTEL_SERVICE_NAME` and `OTEL_EXPORTER_OTLP_ENDPOINT` by environment variable. The docs explicitly say this works without changing your FastMCP code. ([FastMCP][1])

Programmatic SDK setup is also supported, but the telemetry docs are explicit that the tracer provider must be configured **before importing FastMCP** so FastMCP initializes against the intended provider. That import-order rule is the most important implementation detail for code-based telemetry bootstrapping. ([FastMCP][1])

```python id="d8plhm"
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter

provider = TracerProvider()
provider.add_span_processor(
    BatchSpanProcessor(OTLPSpanExporter(endpoint="http://localhost:4317"))
)
trace.set_tracer_provider(provider)

from fastmcp import FastMCP

mcp = FastMCP("my-server")
```

### 29.2 Span model: what FastMCP traces by default

The telemetry docs define server spans for the core MCP operations using names like `tools/call {name}`, `resources/read {uri}`, and `prompts/get {name}`. The same page says the FastMCP client generates outgoing spans using the same naming pattern, which gives you end-to-end request visibility across client and server. ([FastMCP][1])

Mounted composition and proxying show up in the trace tree. For mounted child servers, the docs say FastMCP inserts an additional `delegate {name}` span so the handoff to the child server is visible. For proxy providers, the documented span hierarchy shows the proxy-provider server span and then continuation into remote-server spans through trace-context propagation. That means provider composition is visible in traces by design, not hidden behind a monolithic “tool call” span. ([FastMCP][1])

The telemetry page also documents FastMCP-specific attributes such as `fastmcp.server.name`, `fastmcp.component.type`, `fastmcp.component.key`, `fastmcp.provider.type`, and provider-delegation attributes like `fastmcp.delegate.original_name` / `fastmcp.delegate.original_uri` and `fastmcp.proxy.backend_name` / `fastmcp.proxy.backend_uri`. These are the attributes to key on when building dashboards for composition-heavy servers. ([FastMCP][1])

### 29.3 Custom spans and error behavior

FastMCP exposes `fastmcp.telemetry.get_tracer()` so application code can create custom spans inside tools or other server components. The telemetry docs show this explicitly and use it to wrap internal phases like parsing and processing while attaching custom attributes. That is the correct way to enrich traces with domain-specific timing without replacing FastMCP’s built-in spans. ([FastMCP][1])

```python id="jvj7m5"
from fastmcp import FastMCP
from fastmcp.telemetry import get_tracer

mcp = FastMCP("custom-spans")

@mcp.tool()
async def complex_operation(input: str) -> str:
    tracer = get_tracer()

    with tracer.start_as_current_span("parse_input") as span:
        span.set_attribute("input.length", len(input))
        parsed = parse(input)

    with tracer.start_as_current_span("process_data") as span:
        span.set_attribute("data.count", len(parsed))
        result = process(parsed)

    return result
```

Error recording is automatic. The telemetry docs say when a tool raises, the span is marked with `ERROR` status and an exception event with stack trace is recorded. That makes built-in tracing enough to answer “what failed, where, and under which request/component/provider path?” even before adding custom span logic. ([FastMCP][1])

### 29.4 Telemetry testing

FastMCP documents an explicit telemetry-testing pattern using OpenTelemetry’s in-memory exporter. The page shows a pytest fixture that installs a `TracerProvider` with `SimpleSpanProcessor(InMemorySpanExporter())`, swaps it into `trace.set_tracer_provider(...)`, and then asserts against spans generated by a real FastMCP call. This is the correct test style when you need deterministic assertions over span names, status, and attributes without standing up Jaeger, Tempo, or OTLP collectors. ([FastMCP][1])

Use the in-memory exporter for unit tests that verify trace creation, naming, and error marking. Use a real collector only for integration smoke tests covering exporter wiring, collector reachability, and backend-specific dashboards. The docs do not state that recommendation verbatim, but it follows directly from their in-memory-exporter fixture for tests and OTLP collector guidance for runtime export. ([FastMCP][1])

### 29.5 CLI inspection as the primary schema/manifest verification tool

`fastmcp inspect` is the core inspection and manifest-verification command. The CLI docs say it supports the same **local** entrypoint forms as `fastmcp run`—inferred instance, explicit `file.py:object`, factory function, and `fastmcp.json`—but does **not** inspect remote URLs or generic MCP config files. That means `inspect` is for verifying a server definition you control locally, not for interrogating arbitrary remote MCP services. ([FastMCP][2])

The most important inspection split is output format. `--format fastmcp` emits the full FastMCP-specific manifest, including tags, enabled state, annotations, custom metadata, and output schemas. `--format mcp` emits the protocol view—the exact standard MCP fields clients will see, in camelCase, without FastMCP-only extras. The docs explicitly position `fastmcp` format for internal introspection/debugging and `mcp` format for client-visibility and compatibility verification. ([FastMCP][3])

```bash id="mokczt"
fastmcp inspect server.py
fastmcp inspect server.py --format fastmcp
fastmcp inspect server.py --format mcp -o manifest.json
fastmcp inspect fastmcp.json
```

Operationally, this gives you two verification gates:

* **full-framework manifest verification** with `--format fastmcp`
* **wire-surface verification** with `--format mcp`

Run both in CI when schema drift matters. The docs’ format split is exactly what makes this two-layer verification possible. ([FastMCP][3])

### 29.6 Schema verification in tests

FastMCP’s testing guide explicitly recommends inline snapshots for complex data structures and calls out JSON schemas and API responses as prime use cases. The page shows `inline-snapshot` being used to pin an expected schema shape and says `pytest --inline-snapshot=create` populates empty snapshots while `pytest --inline-snapshot=fix` updates them intentionally. This is the most direct documented pattern for guarding against accidental schema or manifest drift. ([FastMCP][4])

That suggests a clean test strategy:

* use `fastmcp inspect --format mcp` in CI for whole-server manifest verification
* use inline snapshots in unit tests for per-component schema/output verification. ([FastMCP][3])

### 29.7 Testing strategy: in-memory first

FastMCP’s testing docs say **in-memory transport** should cover most unit-testing needs. Pass the server instance directly to `Client(server)`, run everything in the same process, and test real MCP protocol behavior without network overhead. The docs explicitly describe this as deterministic, fast, and debugger-friendly, with typical completion in milliseconds. ([FastMCP][4])

```python id="kxjlwm"
from fastmcp import FastMCP, Client

server = FastMCP("WeatherServer")

@server.tool
def get_temperature(city: str) -> dict:
    temps = {"NYC": 72, "LA": 85, "Chicago": 68}
    return {"city": city, "temp": temps.get(city, 70)}

async def test_weather_operations():
    async with Client(server) as client:
        result = await client.call_tool("get_temperature", {"city": "NYC"})
        assert result.data == {"city": "NYC", "temp": 72}
```

For most application code, this should be the default test harness. Reach for network transports only when you need to verify actual HTTP/SSE transport behavior, reverse-proxy assumptions, auth headers, or process/isolation semantics. That recommendation is a direct reading of the test guide’s “in-memory covers most unit testing” statement. ([FastMCP][4])

### 29.8 Testing network transports

When you do need real transport coverage, FastMCP’s testing guide says the preferred approach is **in-process async network testing** with `run_server_async(...)`. The docs explicitly call this preferred over subprocess testing because it is faster, deterministic, and gives better debugger/error behavior. ([FastMCP][4])

```python id="hvz3zc"
import pytest
from fastmcp import FastMCP, Client
from fastmcp.client.transports import StreamableHttpTransport
from fastmcp.utilities.tests import run_server_async

def create_test_server() -> FastMCP:
    server = FastMCP("TestServer")

    @server.tool
    def greet(name: str) -> str:
        return f"Hello, {name}!"

    return server

@pytest.fixture
async def http_server() -> str:
    server = create_test_server()
    async with run_server_async(server) as url:
        yield url

async def test_http_transport(http_server: str):
    async with Client(transport=StreamableHttpTransport(http_server)) as client:
        assert await client.ping() is True
        greeting = await client.call_tool("greet", {"name": "World"})
        assert greeting.data == "Hello, World!"
```

Use subprocess testing only for the cases the docs call out as special: STDIO transport itself, full process isolation, or subprocess-specific behavior. The test guide explicitly treats subprocess transport tests as a separate, heavier class of tests. ([FastMCP][4])

### 29.9 General testing discipline from the FastMCP docs

FastMCP’s test guide is unusually prescriptive, and the recommendations are worth carrying over directly into application codebases. The page says tests should generally complete in under one second unless marked as integration tests; every test should be self-contained; fixtures should not open FastMCP clients because that can create difficult event-loop issues; and good tests verify one behavior clearly rather than many unrelated behaviors at once. ([FastMCP][4])

The same page also demonstrates auth testing by pairing a protected server with `Client(..., auth=BearerAuth(...))`, which is the right pattern when validating authentication-dependent behavior without needing a full external identity provider. Use that pattern for auth unit tests, and reserve full OAuth/browser/callback tests for higher-level integration suites. ([FastMCP][4])

### 29.10 Production checklist

Treat the following as the baseline production hardening checklist for FastMCP:

* **Prefer Streamable HTTP over SSE** for new deployments. The running and client-transport docs both explicitly say SSE is only for backward compatibility and that Streamable HTTP is the recommended path for new HTTP deployments. ([FastMCP][5])
* **Keep `mcp.run()` under `if __name__ == "__main__":`** in ordinary server scripts. The running guide explicitly calls this best practice for compatibility so the server starts only when the script is executed directly, not when imported. ([FastMCP][5])
* **Pin `prefab-ui` for app deployments.** The Apps overview explicitly says FastMCP intentionally does not pin an upper bound for `prefab-ui`, that Prefab changes rapidly, and that production deployments must pin a specific version themselves. ([FastMCP][6])
* **Do not apply naive response caching to auth/session-derived results.** The middleware docs explicitly warn that cache keys are based only on operation name and arguments, not on user or session identity, so tools whose effective results depend on auth headers or session state should not be cached unless identity is part of the arguments. ([FastMCP][7])
* **Use Redis/Valkey for durable or scaled background-task deployments.** The background-task docs explicitly recommend Redis (or Valkey) for production because tasks survive restarts, pickup latency is much lower, and additional workers can scale across processes or machines. ([FastMCP][8])
* **Use ToolSearch when very large tool catalogs would waste context tokens and hurt tool selection.** The ToolSearch docs explicitly say large catalogs waste tokens and degrade tool-selection accuracy, and that the transform replaces a full tool dump with `search_tools` plus `call_tool` so tools are discovered on demand instead of loaded wholesale into context. ([FastMCP][9])
* **Use in-memory transport for most unit tests and `run_server_async(...)` for most network-transport tests.** FastMCP’s testing docs explicitly describe these as the preferred default patterns. ([FastMCP][4])
* **Use `fastmcp inspect --format mcp` as the final “what will clients actually see?” gate.** The inspect docs explicitly define MCP format as the exact protocol view, distinct from the richer FastMCP internal format. ([FastMCP][3])
* **Enable OpenTelemetry export in production rather than relying on no-op instrumentation.** FastMCP instrumentation is always present but only emits spans when an SDK/exporter is configured; the docs explicitly recommend `opentelemetry-instrument` or programmatic SDK setup. ([FastMCP][1])

### 29.11 Closing operational pattern

A practical production loop, directly supported by the docs, is: define the server; verify the public surface with `fastmcp inspect --format mcp`; test behavior primarily with in-memory clients and secondarily with `run_server_async(...)`; run under Streamable HTTP; enable OpenTelemetry export; pin UI dependencies if apps are present; keep response caching limited to identity-stable operations; and move background tasks onto Redis/Valkey before you care about durability or scale. ([FastMCP][3])

If you want, I can now produce the appendix/reference unit next: decorator-argument matrices, DI injection matrix, task-mode truth tables, provider/transform comparison grids, client transport/auth decision matrices, and output-schema/content-block conversion cheat sheets.

[1]: https://gofastmcp.com/servers/telemetry "OpenTelemetry - FastMCP"
[2]: https://gofastmcp.com/cli/inspecting "Inspecting Servers - FastMCP"
[3]: https://gofastmcp.com/v2/patterns/cli "FastMCP CLI - FastMCP"
[4]: https://gofastmcp.com/development/tests "Tests - FastMCP"
[5]: https://gofastmcp.com/deployment/running-server "Running Your Server - FastMCP"
[6]: https://gofastmcp.com/apps/overview "Apps - FastMCP"
[7]: https://gofastmcp.com/servers/middleware "Middleware"
[8]: https://gofastmcp.com/v2/servers/tasks "Background Tasks"
[9]: https://gofastmcp.com/servers/transforms/tool-search "Tool Search"

# FastMCP Advanced — 30) Testing, contract verification, and tool fingerprinting
### Testing, contract verification, and tool fingerprinting

### 30.0 Testing model

A production FastMCP server has multiple contracts. Test all of them separately:

```text
Python declaration contract
  -> server-resolution contract
      -> MCP protocol contract
          -> transport/auth/session contract
              -> side-effect/business contract
```

A unit test that calls the underlying Python function proves only the last layer of local business logic. It does **not** prove schema generation, dependency injection, middleware, provider transforms, visibility, auth, transport initialization, or MCP result conversion.

---

## 30.1 Test pyramid

| Layer | Recommended technique | Catches |
| --- | --- | --- |
| Pure domain | Ordinary pytest unit tests | Business logic, edge cases |
| Component declaration | `mcp.get_tool`, resource/prompt lookup | Metadata, version, generated schema |
| In-memory MCP | `Client(mcp)` | Full MCP request pipeline without network |
| Manifest | `fastmcp inspect --format mcp` | Published protocol contract drift |
| Transport integration | STDIO/HTTP test server | Serialization, process/network lifecycle |
| Auth/security | HTTP client with tokens/claims | Authentication and authorization policy |
| Host/App | Real target client / Apps preview | Host-specific capabilities and UI |

---

## 30.2 In-memory client is the default integration test primitive

```python
import pytest
from fastmcp import Client, FastMCP

mcp = FastMCP("Calculator")

@mcp.tool
def add(a: int, b: int) -> int:
    return a + b

@pytest.mark.asyncio
async def test_add_over_mcp():
    async with Client(mcp) as client:
        result = await client.call_tool("add", {"a": 2, "b": 3})
        assert result.data == 5
```

This exercises:

* generated tool schema;
* argument validation;
* FastMCP call routing;
* sync/async execution adaptation;
* result serialization;
* client result parsing.

Use direct function tests **in addition** to this, not instead of it.

---

## 30.3 Catalog tests

```python
@pytest.mark.asyncio
async def test_catalog():
    async with Client(mcp) as client:
        tools = await client.list_tools()
        names = {t.name for t in tools}
        assert "add" in names
        assert "internal_debug" not in names
```

Catalog tests should verify:

```text
[ ] expected public components exist
[ ] internal/app-only components do not leak
[ ] namespaces are correct
[ ] highest intended version is selected
[ ] disabled versions fall back as intended
[ ] auth-filtered tools appear/disappear for the right principal
[ ] search transforms expose only the intended discovery tools
```

---

## 30.4 Schema assertions

Inspect the protocol-facing representation, not an internal Pydantic model alone.

Conceptual server-side path:

```python
async def get_protocol_tool(server: FastMCP, name: str):
    tool = await server.get_tool(name)
    assert tool is not None
    return tool.to_mcp_tool()
```

Validate:

* required argument list;
* enum/literal values;
* field descriptions;
* defaults;
* array/object structure;
* `outputSchema` where relied upon;
* annotations that influence routing/safety UX;
* version/name identity.

---

## 30.5 Tool fingerprinting: stable contract hash recipe

FastMCP deliberately does not prescribe a single universal hash because different applications care about different metadata. It exposes the pieces needed to build one:

```text
tool.key          -> canonical FastMCP component identity (type/name/version)
tool.to_mcp_tool  -> protocol-facing object clients receive
```

Reference implementation:

```python
import hashlib
import json
from fastmcp import FastMCP

async def fingerprint_tool(server: FastMCP, tool_name: str) -> str:
    tool = await server.get_tool(tool_name)
    if tool is None:
        raise KeyError(tool_name)

    wire = tool.to_mcp_tool().model_dump(
        mode="json",
        by_alias=True,
        exclude_none=True,
    )

    payload = {
        "key": tool.key,
        "inputSchema": wire["inputSchema"],
        "outputSchema": wire.get("outputSchema"),
    }

    canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()
```

The inclusion policy is yours.

---

## 30.6 What belongs in a fingerprint?

| Field | Include when |
| --- | --- |
| `tool.key` | Always; captures type/name/version identity |
| `inputSchema` | Always for callable contract drift |
| `outputSchema` | Downstream code validates structured results |
| `description` | Tool selection/routing behavior depends on wording |
| `annotations` | Read-only/destructive/idempotent hints drive policy |
| `_meta` | Custom metadata participates in authorization/routing |
| tags | Publication/policy systems consume tags |
| icons/title | Usually exclude unless UI compatibility is contract-critical |

Do not hash volatile runtime values such as timestamps or deployment IDs into a contract fingerprint.

---

## 30.7 Manifest generation

```python
async def tool_manifest(server: FastMCP) -> dict[str, str]:
    result = {}
    for tool in await server.list_tools():
        result[tool.key] = await fingerprint_tool(server, tool.name)
    return result
```

Store the manifest as a CI artifact:

```json
{
  "tool:add@1.0": "1c2d...",
  "tool:lookup_customer@2.1": "c9e4..."
}
```

Review changes rather than blindly accepting them.

---

## 30.8 Drift classification

Not all changes have the same risk.

| Change | Typical risk |
| --- | --- |
| add optional parameter with safe default | low/medium |
| change description only | routing behavior can change even if schema-compatible |
| make optional parameter required | breaking |
| rename parameter | breaking |
| narrow enum | breaking for callers using removed value |
| widen enum | usually compatible, but downstream validation may differ |
| change output object shape | potentially breaking |
| bump tool version and keep old version | migration-friendly |
| remove tool/version | breaking for callers pinned to it |

Treat model-facing description changes as real behavior changes even when traditional API compatibility tooling would ignore them.

---

## 30.9 Snapshot testing protocol manifests

`fastmcp inspect server.py --format mcp` gives a protocol-oriented manifest suitable for snapshot/golden testing.

```bash
fastmcp inspect server.py --format mcp -o current.json
python scripts/compare_manifest.py baseline.json current.json
```

Keep snapshots deterministic:

* canonicalize JSON ordering;
* ignore known volatile metadata;
* pin FastMCP version;
* pin provider/generated-spec inputs;
* separate expected schema changes from incidental dependency drift.

---

## 30.10 Version behavior tests

For multiple tool versions:

```python
@mcp.tool(name="search", version="1.0")
def search_v1(query: str): ...

@mcp.tool(name="search", version="2.0")
def search_v2(query: str, limit: int = 20): ...
```

Test:

```text
default lookup -> 2.0
explicit version 1.0 -> v1 implementation
v2 disabled -> default falls back to enabled v1
unknown explicit version -> not found/error
```

Do the same for versioned prompts/resources when they are part of your compatibility strategy.

---

## 30.11 Visibility and authorization matrix tests

A useful fixture matrix:

| Principal/session | Expected catalog |
| --- | --- |
| anonymous | public read-only tools |
| standard user | public + user-scoped tools |
| admin | admin tools additionally visible |
| app renderer | app-visible backend tools only as intended |
| search-mode client | search/execute meta-tools, not giant raw catalog |

Test both:

1. **listing visibility**;
2. **direct call enforcement**.

A hidden tool that remains callable without authorization is not protected; a visible tool that is denied at execution may be acceptable depending on UX but should be intentional.

---

## 30.12 Middleware tests

Create a trace list in a test middleware and assert ordering:

```text
outer before
  inner before
    handler
  inner after
outer after
```

Test exception paths too. Error middleware that only works on successful responses is not an error middleware.

For caching middleware, specifically test identity-sensitive operations: two users calling the same operation with the same arguments must never receive each other's result if the cache key does not incorporate identity.

---

## 30.13 Context/DI tests

Test that runtime-only dependencies:

* are absent from the MCP schema;
* resolve exactly once per request where caching is intended;
* clean up context-manager dependencies;
* see the correct access token/request/server;
* fail clearly when used outside a request.

A schema regression test should explicitly ensure secrets/DB session parameters never become model-visible.

---

## 30.14 Background task tests

Test the full state machine:

```text
submit -> task id
status -> queued/running
progress -> monotonic/meaningful
result -> final value
cancel -> cancelled where supported
worker restart -> durability behavior matches backend design
```

Do not test only the task function directly. The task transport/result lifecycle is the feature.

---

## 30.15 Transport parity tests

When you support more than one transport, run the same contract suite through:

```text
in-memory
STDIO
Streamable HTTP
```

This catches:

* stdout contamination;
* process environment differences;
* URL/path/mount mistakes;
* auth-only-on-HTTP assumptions;
* lifespan/session differences;
* reverse-proxy metadata issues.

SSE should only be in the matrix when you intentionally support legacy clients.

---

## 30.16 OpenAPI/generated-provider tests

For generated components, pin the source spec and verify:

```text
route count
excluded operation ids
public tool names
input schemas
outbound URL/method/query/body
auth header mapping
error conversion
SSRF/path traversal cases
```

A changed upstream OpenAPI document is an MCP contract change even when no FastMCP Python source changed.

---

## 30.17 Apps tests

Beyond the tool result:

```text
[ ] app metadata links to a ui:// renderer
[ ] structuredContent is serializable
[ ] app-only backend tools are not model-visible
[ ] namespacing does not break backend calls
[ ] unauthorized backend action denied
[ ] file upload scope stable in target deployment mode
[ ] custom CSP/permissions correct
```

Run browser/host-level tests for critical UIs; in-memory MCP alone cannot prove iframe rendering behavior.

---

## 30.18 Production smoke tests

After deployment:

```bash
fastmcp list https://staging.example.com/mcp
fastmcp call https://staging.example.com/mcp health --json
```

Also verify:

* OAuth discovery endpoints;
* token audience/issuer handling;
* mounted path correctness;
* Host/Origin behavior behind the real proxy;
* stream buffering disabled where needed;
* task/event-store backend reachable.

---

## 30.19 Anti-pattern inventory

* Testing only decorated Python functions directly.
* Snapshotting internal Python object reprs instead of MCP wire models.
* Ignoring descriptions because “only schemas matter.”
* Hashing unsorted JSON and calling it a stable fingerprint.
* Accepting manifest drift automatically in CI.
* Testing catalog visibility but not direct authorization.
* Testing auth only in-memory when production auth exists only over HTTP.
* Testing Apps only as JSON and never in the target host.
* Treating an OpenAPI spec update as non-code/non-contract change.

---

## 30.20 Agent checklist

```text
[ ] Unit-test domain functions separately from MCP integration.
[ ] Make Client(mcp) the default MCP integration-test path.
[ ] Snapshot/inspect the protocol-facing manifest.
[ ] Fingerprint tool.key + selected wire fields deterministically.
[ ] Classify contract drift by compatibility risk.
[ ] Test versions, visibility, and auth matrices.
[ ] Test middleware ordering and failure paths.
[ ] Test DI parameters remain hidden.
[ ] Test task lifecycle, not just task body.
[ ] Run real transport tests for every supported deployment transport.
[ ] Test generated-provider outbound HTTP behavior.
[ ] Run host-level tests for critical Apps.
```

### Sources

1. https://gofastmcp.com/servers/testing
2. https://gofastmcp.com/servers/tool-fingerprinting
3. https://gofastmcp.com/cli/inspecting
4. https://gofastmcp.com/clients/client
5. https://gofastmcp.com/servers/versioning
6. https://gofastmcp.com/servers/visibility

# FastMCP Advanced — 31) Ecosystem and host integrations
### Ecosystem and host integrations

### 31.0 Integration stance

FastMCP implements MCP server/client capabilities, but the **host decides which MCP capabilities are actually surfaced to the user/model**. A server that is valid MCP can behave differently across Claude Desktop, Claude Code, Cursor, Gemini CLI, Goose, ChatGPT, custom clients, or agent frameworks because hosts differ in transport support, Apps rendering, elicitation, sampling, roots, auth UX, and catalog handling.

Therefore document two things separately:

```text
FastMCP capability
vs
Target host capability
```

Never infer host support from FastMCP support alone.

---

## 31.1 Host integration matrix: questions to answer

For every target host, record:

| Dimension | Question |
| --- | --- |
| Transport | STDIO, Streamable HTTP, both? |
| Auth | OAuth/DCR/CIMD/bearer UX supported? |
| Apps | MCP Apps renderer supported? |
| Elicitation | v3 client-mediated elicitation supported? |
| Sampling | Can server request model sampling in the v3 protocol era? |
| Roots | Does the client expose roots? |
| Tasks | Does the client understand task semantics you rely on? |
| Tool count | Does it ingest the entire catalog well? |
| Resources/prompts | Are non-tool component types surfaced? |
| Logging/progress | Are notifications displayed usefully? |

Pin those answers to a host version where possible because client capabilities evolve independently of FastMCP.

---

## 31.2 Claude Desktop / Claude Code

Common patterns:

* local STDIO servers launched as subprocesses;
* remote HTTP servers for centralized deployments;
* configuration/installation via client-specific MCP mechanisms;
* server dependencies must be explicit because host-launched processes may not inherit your development virtual environment.

FastMCP CLI provides install/discovery helpers for both environments. Prefer explicit portable config/commands rather than hand-editing opaque launcher strings repeatedly.

### Claude Code and agent workflows

`fastmcp generate-cli` can emit `SKILL.md`, which can be useful when a coding agent should operate through a generated CLI rather than dynamically inspecting MCP on every turn. This is complementary to direct MCP integration, not a replacement for it.

---

## 31.3 Cursor

Cursor commonly consumes MCP server configuration at the workspace/user level. Practical concerns:

* working-directory-relative paths;
* correct Python/uv environment;
* STDIO command quoting;
* remote URL/auth support;
* tool catalog size.

For large generated catalogs, ToolSearch or Code Mode can be more important than in small demo servers because IDE agents already carry substantial code context and benefit from not injecting hundreds of tool schemas up front.

---

## 31.4 Gemini CLI

Treat Gemini CLI like any other MCP host: verify the actual transport/auth/capability surface rather than relying on server assumptions. FastMCP's CLI can discover/install configurations for supported versions, which makes it practical to test the same server across multiple hosts using one project artifact.

---

## 31.5 Goose

Goose is a useful target for Apps testing because it supports interactive app workflows in documented FastMCP examples. Still test the exact app/provider features you use—especially file upload, approval/choice, and custom renderer behavior—rather than assuming one rendered demo proves complete Apps compatibility.

---

## 31.6 ChatGPT and remote MCP

For remote-host integrations, the decisive requirements are usually:

```text
public/reachable HTTPS URL
correct Streamable HTTP path
OAuth/auth metadata if protected
stable certificate/proxy routing
client-compatible component catalog
```

Do not expose a local development server directly to the Internet merely to make a host reach it. Put the service behind an intentional deployment boundary with authentication, TLS, logging, rate limits, and network policy.

---

## 31.7 Generic MCP JSON configuration

When a client is not directly supported by FastMCP's install helper, generate or hand off standard MCP configuration rather than embedding host-specific assumptions into server code.

Separation:

```text
server.py / package        -> server implementation
fastmcp.json               -> FastMCP project/deployment contract
mcp.json / host config     -> how a particular MCP host launches/connects
```

Do not make server business code read Cursor/Claude/Gemini configuration files.

---

## 31.8 FastAPI integration

FastAPI appears in two ecosystem roles:

1. **source**: `FastMCP.from_fastapi(app)` generates MCP components from FastAPI/OpenAPI;
2. **host web framework**: mount `mcp.http_app()` into a FastAPI application.

These are deliberately distinct. Section 26 covers generation; Sections 19–20 cover mounting/lifespan/reverse-proxy deployment.

---

## 31.9 Agent-framework integration

FastMCP's `Client` is a deterministic MCP consumer and is often the best integration boundary for higher-level agents. Framework code can:

```python
from fastmcp import Client

async with Client("https://example.com/mcp") as client:
    tools = await client.list_tools()
    result = await client.call_tool("search", {"query": "..."})
```

This preserves control over:

* session lifecycle;
* retries/timeouts;
* auth;
* handler wiring;
* result parsing;
* orchestration policy.

If an agent framework already has native MCP support, decide whether to use that integration or FastMCP Client based on the needed auth/transport/handler feature set. Avoid stacking two MCP client abstractions unless one intentionally wraps the other.

---

## 31.10 OpenAI / Anthropic / Gemini model APIs and sampling

FastMCP v3 supports server-initiated sampling through the connected MCP client when the client exposes that capability. This is **not the same** as calling an LLM provider SDK directly inside your tool.

```text
ctx.sample(...)
  -> asks the connected MCP client to perform model sampling

provider_sdk.chat(...)
  -> server itself owns provider credentials/model call
```

Choose intentionally:

| Need | Prefer |
| --- | --- |
| Borrow the host's current model/context | v3 MCP sampling |
| Server owns a fixed model/provider/credential | provider SDK in server |
| Deterministic business action | neither; ordinary code/tool |

FastMCP 4's modern sessionless protocol removes server-initiated sampling on modern connections; this is a major migration decision covered in Section 36.

---

## 31.11 Pydantic AI and other Python agent frameworks

The general integration pattern is to use MCP as an external capability boundary and keep framework-specific agent state outside the FastMCP server unless the server genuinely owns that state.

Avoid:

* leaking framework-internal conversation objects into tool schemas;
* coupling server startup to one agent framework unnecessarily;
* returning provider-specific response objects directly from tools without explicit serialization.

Prefer plain typed Python models at the MCP boundary.

---

## 31.12 Filesystem / skills providers as ecosystem adapters

Provider abstractions let FastMCP treat filesystem content, remote MCP servers, skills, OpenAPI APIs, and custom dynamic catalogs as **sources** rather than hardcoded local decorators.

This makes an integration architecture compositional:

```text
local business tools
+ remote finance MCP proxy
+ filesystem docs provider
+ skills provider
+ OpenAPI provider
  -> transforms/namespaces
  -> one curated server surface
```

Every added provider expands the trust boundary. Apply namespacing, auth, visibility, and failure isolation deliberately.

---

## 31.13 Remote bridges and protocol translation

Use `fastmcp-remote` when a client only knows how to spawn STDIO but the actual server is remote HTTP. Use a programmatic `Client` when your Python code consumes MCP itself. Use a FastMCP proxy/gateway when you want to **republish** a remote MCP surface as part of another server.

Do not conflate:

```text
remote bridge = transport adaptation for a client
Client        = programmatic consumer
proxy server  = republished server surface
```

---

## 31.14 Capability negotiation strategy

For optional client features, fail gracefully:

```text
client supports feature -> use richer interaction
client lacks feature     -> return a plain fallback or reject clearly
```

Examples:

* Apps -> structured/plain text fallback where practical;
* elicitation -> require all fields up front if client cannot elicit;
* sampling -> use configured server model or disable feature if no sampling handler;
* roots -> accept explicit path/resource parameters if roots unavailable.

Do not silently produce incorrect behavior just to avoid a capability error.

---

## 31.15 Cross-host acceptance suite

For every supported host:

```text
[ ] connect / initialize
[ ] list tools/resources/prompts
[ ] call simple typed tool
[ ] structured output rendered/returned correctly
[ ] auth flow succeeds
[ ] long response / streaming works
[ ] progress/log behavior acceptable
[ ] elicitation/sampling/roots tested if required
[ ] Apps render and backend calls work if required
[ ] restart/reconnect behavior acceptable
[ ] tool catalog size remains usable
```

Store the host name/version in test results.

---

## 31.16 Anti-pattern inventory

* “FastMCP supports it, therefore every MCP host supports it.”
* Host-specific config logic inside business tools.
* Assuming the host inherits your shell virtualenv.
* Requiring Apps without a non-App compatibility story when multiple hosts are supported.
* Requiring sampling without testing the client handler/capability.
* Exposing hundreds of tools to an IDE agent without catalog strategy.
* Using a transport bridge when what you actually need is a proxy server—or vice versa.
* Publishing an unauthenticated Internet endpoint just to satisfy a remote host.

---

## 31.17 Agent checklist

```text
[ ] Record supported MCP hosts and versions.
[ ] Record transport/auth/App/elicitation/sampling/roots capability per host.
[ ] Keep host config outside server business logic.
[ ] Declare runtime dependencies for host-launched STDIO servers.
[ ] Test remote HTTP through the real TLS/proxy path.
[ ] Use search/code-mode for large catalogs where host tool loading is expensive.
[ ] Provide fallbacks for optional host capabilities when product requirements allow.
[ ] Distinguish Client, fastmcp-remote, and proxy-server use cases.
[ ] Run a cross-host acceptance suite before claiming interoperability.
```

### Sources

1. https://gofastmcp.com/cli/install-mcp
2. https://gofastmcp.com/cli/client
3. https://gofastmcp.com/clients/client
4. https://gofastmcp.com/clients/fastmcp-remote
5. https://gofastmcp.com/integrations
6. https://gofastmcp.com/apps/overview

# FastMCP Advanced — 32) Security hardening and governance
### Security hardening and governance

### 32.0 Threat-model-first stance

FastMCP provides security primitives; it does not know your business authorization model. A production threat model should separate at least these boundaries:

```text
1. Network/transport boundary
2. MCP authentication boundary
3. Component authorization boundary
4. Publication/discovery boundary
5. Business-action boundary
6. Outbound-provider/proxy boundary
7. File/code/UI execution boundary
8. State/cache/task persistence boundary
```

Security failures often happen when one layer is mistaken for another—for example, using hidden visibility as authorization or using an Approval card as enforcement.

---

## 32.1 Security control map

| Threat/control goal | FastMCP mechanism | Still your responsibility |
| --- | --- | --- |
| Identify HTTP caller | auth provider / token verification | IdP policy, credentials, tenant model |
| Limit component access | component/server authorization checks | Business roles/ownership rules |
| Hide irrelevant components | visibility/transforms | Do not treat hiding as security |
| Prevent transport spoofing | Host/Origin protection, TLS/proxy config | Correct proxy trust/network ACLs |
| Prevent upstream SSRF | hardened proxy/OpenAPI behavior | Allowlisted origins and argument design |
| Prevent cross-user cache leak | identity-aware cache policy | Include principal/tenant in cache partition |
| Prevent file escape | resource/upload path policy | Storage sandbox, filename normalization |
| Prevent arbitrary model code effect | Code Mode/Generative UI sandbox | Backend-tool auth + sandbox policy |
| Protect long-running state | storage/task backend | Encryption, retention, tenancy, backups |

---

## 32.2 Authentication is HTTP-scoped in v3

FastMCP token/OAuth authentication is relevant to HTTP/SSE request flows. STDIO is normally secured by the local process/launcher/OS boundary.

Implication:

```text
HTTP remote server -> FastMCP auth can authenticate network callers
STDIO child process -> secure executable/config/env/filesystem and host process
```

Do not assume `FastMCP(auth=...)` protects a local STDIO server from a malicious local process that can launch/invoke it.

---

## 32.3 Authentication vs authorization

Authentication answers:

```text
Who is this request?
```

Authorization answers:

```text
May this principal execute/read/render this specific component/action?
```

Use component-level checks (`auth=...`) or centralized middleware/policy for scopes/roles/claims. Inside business actions, enforce object-level ownership too.

Example:

```text
scope says invoices:write
AND invoice.tenant_id == current_principal.tenant_id
AND invoice.status allows transition
```

Scope alone rarely captures object-level business authorization.

---

## 32.4 Visibility is not authorization

Visibility transforms control what gets listed or discovered. They improve UX, token economy, and accidental exposure, but direct invocation must still be rejected when unauthorized.

Test both:

```text
unauthorized user list_tools -> secret tool absent
unauthorized user call secret tool by known name -> denied
```

If the second test succeeds, the system is insecure even if the first test passes.

---

## 32.5 Search/Code Mode is not a security sandbox for the catalog

Search transforms hide the full catalog from the model and expose discovery/execute meta-tools. This is excellent for scale but can create a false sense of safety.

Correct order:

```text
provider/source
  -> authorization/visibility-safe component set
      -> search/indexing transform
          -> model discovery
```

Never index sensitive unauthorized components and rely on the model “not finding them.”

---

## 32.6 HTTP Host/Origin hardening

FastMCP 3.4 added/hardened checks around Host/Origin and proxy deployments in response to DNS rebinding and deployment-compatibility concerns.

Production guidance:

* configure trusted external hostnames explicitly;
* understand what headers your reverse proxy rewrites;
* do not trust arbitrary `X-Forwarded-*` unless requests come from a trusted proxy;
* test the real deployment path, not only direct localhost;
* reject unexpected Origins/Hosts where the deployment model allows;
* use TLS at the public boundary.

A security feature disabled “because nginx made it fail” should be replaced by correct trusted-proxy configuration, not removed reflexively.

---

## 32.7 OAuth discovery and redirect integrity

Mounted OAuth-protected servers need correct root-level `.well-known` discovery and consistent public URLs.

Security invariants:

```text
base_url + mcp_path == actual public MCP URL
issuer metadata matches intended authorization server
redirect/callback URLs are predeclared or tightly validated
.well-known routes are available at standards-required paths
```

Do not derive OAuth redirect destinations from arbitrary untrusted request headers without a trusted-proxy/public-URL policy.

---

## 32.8 CIMD / OAuth client metadata

Client ID Metadata Documents (CIMD) allow OAuth clients to publish metadata at a URL-like client identifier. FastMCP 3.4.x includes hardening around audience validation and metadata/JWKS retrieval.

Threat model:

```text
untrusted URL metadata fetch -> SSRF / internal network reachability
malicious metadata/JWKS -> identity confusion
bad audience handling -> token accepted for wrong resource
```

Keep metadata/JWKS fetch protections enabled and only relax them through explicit trusted-proxy/network policy.

---

## 32.9 Proxy-provider credential separation

A gateway may have:

1. caller credential for the public FastMCP gateway;
2. gateway service/delegated credential for the upstream MCP server.

Never blindly copy the inbound Authorization header to the upstream target.

Explicit mapping:

```text
validated incoming principal
  -> authorization decision
  -> choose upstream identity strategy
      service token OR scoped token exchange OR delegated credential
```

This prevents confused-deputy behavior and accidental cross-environment credential leakage.

---

## 32.10 OpenAPI/HTTP-provider SSRF

Generated API tools can turn seemingly harmless string parameters into outbound paths/URLs. Defend with:

* fixed/allowlisted upstream origins;
* safe URL joining;
* path normalization;
* forbidden local/link-local ranges where not needed;
* redirect controls;
* allowlisted schemes;
* route exclusions;
* patched FastMCP version;
* adversarial tests.

Treat every provider that performs outbound network fetches as an SSRF boundary.

---

## 32.11 File resources and upload security

For `FileResource`, `DirectoryResource`, filesystem providers, and `FileUpload`:

```text
[ ] absolute/known root
[ ] canonicalize path before access
[ ] reject traversal outside root
[ ] do not trust uploaded filename
[ ] enforce bytes/content limits
[ ] isolate tenants/principals
[ ] avoid following unsafe symlinks when sandboxing matters
[ ] sanitize archive extraction paths
[ ] retention and deletion defined
```

File URI access should never accidentally become “arbitrary filesystem read.”

---

## 32.12 Code Mode security

Code Mode intentionally executes model-generated orchestration code to interact with the server catalog. Its value depends on a sandbox.

Threat model includes:

* filesystem access;
* process spawning;
* network access;
* introspection of secrets/environment;
* resource exhaustion;
* infinite loops;
* bypassing intended tool-level input constraints.

Use the documented sandbox path and keep effectful tool authorization intact. A sandbox constrains the model code environment; it does not authorize backend effects.

---

## 32.13 Generative UI security

Generative UI executes model-authored UI code in isolated browser/server-side sandbox environments. Apply the same principle:

```text
sandbox -> limits arbitrary code effects
backend tool auth -> limits business effects
CSP/permissions -> limits renderer capabilities
```

Do not expose secrets to `structuredContent` just because the renderer is sandboxed; the client still receives that payload.

---

## 32.14 Tool annotations are advisory

`readOnlyHint`, `destructiveHint`, `idempotentHint`, and similar annotations can improve client UX/routing, but they are not security enforcement.

A destructive tool marked `readOnlyHint=True` remains destructive if its implementation deletes data. Security derives from server behavior and authorization, not metadata honesty.

Still keep annotations accurate because safety-oriented clients may rely on them for confirmation UX.

---

## 32.15 Approval is advisory

The built-in Approval app sends a choice back into the conversation. It does not create an unforgeable approval credential by itself.

For hard approval:

```text
user approves through trusted UI/workflow
  -> server records approval bound to principal + action + object + expiry
      -> destructive tool verifies record atomically
          -> consume/mark approval used
```

Do not implement hard approval as `approved: bool` supplied by the LLM.

---

## 32.16 Caching security

The built-in/typical response cache key may be based on operation and arguments rather than user identity. That is dangerous when results depend on:

* current access token;
* session state;
* tenant;
* per-user resource visibility;
* request headers;
* dynamic authorization.

Correct policy:

```text
public deterministic response -> global cache can be okay
principal-dependent response   -> principal/tenant partition or no shared cache
session-dependent response     -> session partition or no shared cache
```

Add explicit tests where two principals call identical arguments.

---

## 32.17 Session state and storage security

Persistent state backends require:

* tenant/user partitioning;
* serialization validation;
* size limits;
* TTL/retention;
* encryption at rest when sensitive;
* access-control around Redis/Valkey/database backend;
* namespace/version migration discipline.

Do not store raw bearer tokens in generic session state unless the architecture explicitly requires it and the storage boundary is appropriate.

---

## 32.18 Background tasks security

A task ID is not necessarily authorization. On every task read/cancel/result action, preserve the principal/tenant relationship.

For durable workers:

```text
submitted principal/tenant/action
  -> serialized task payload
      -> worker re-establishes authorization context or uses scoped service capability
```

Do not let a task run indefinitely with an overprivileged credential copied from an interactive request.

---

## 32.19 Error masking and information disclosure

Use `mask_error_details=True` on Internet-facing production servers unless detailed error propagation is an explicit product requirement.

Still return intentional user-safe errors (`ToolError` or equivalent) when callers need actionable feedback.

Avoid leaking:

* filesystem paths;
* SQL strings with secrets;
* upstream auth headers;
* stack traces;
* internal hostnames/IPs;
* tenant identifiers across users.

Keep full diagnostics in secured server logs/traces.

---

## 32.20 Logging and telemetry hygiene

Audit logs should answer:

```text
who
called what component/version
when
with which tenant/object identifiers
result status / duration
policy decision
trace/request/task id
```

But avoid logging secrets or full sensitive payloads by default. Structured redaction is better than “remember not to log password fields.”

---

## 32.21 Rate limits and resource limits

Authorization does not prevent resource exhaustion by an authorized client.

Control:

* request rate;
* concurrent calls;
* tool timeout;
* upload size;
* result size;
* pagination size;
* background task quota;
* external API quota;
* sandbox CPU/memory/time;
* provider catalog query limits.

Use rate-limiting middleware plus infrastructure-level limits for defense in depth.

---

## 32.22 Dependency and release security

FastMCP's protocol surface evolves quickly and security fixes have shipped throughout 3.x. Production rules:

```text
[ ] pin FastMCP exact stable version
[ ] monitor FastMCP/GitHub security advisories
[ ] update lockfile intentionally
[ ] run contract/security tests on patch/minor upgrades
[ ] pin Prefab for Apps
[ ] patch upstream dependencies and auth libraries
```

Do not run a stale minor just because “the API still works.”

---

## 32.23 Threat matrix

| Threat | Example | Primary control |
| --- | --- | --- |
| Unauthorized tool call | user guesses hidden admin tool name | auth check + object-level authorization |
| Catalog disclosure | user lists internal tools | visibility + authorization filtering |
| SSRF | generated OpenAPI URL hits metadata service | origin/network controls + patched provider |
| DNS rebinding | hostile Host routes browser/local server | Host/Origin protection |
| Cross-user cache | same args returns another user's data | identity partition / no shared cache |
| Path traversal | uploaded `../../secret` | canonicalized root + generated storage key |
| Code escape | Code Mode arbitrary Python | sandbox + backend authorization |
| UI privilege escalation | app calls hidden destructive backend | backend auth; app visibility not enough |
| OAuth confusion | token audience for wrong resource accepted | issuer/audience verification |
| Task theft | guessed task ID retrieves result | task ownership enforcement |

---

## 32.24 Security acceptance checklist

```text
[ ] Threat model each provider and transport.
[ ] Authenticate all remote production callers unless endpoint is intentionally public.
[ ] Enforce authorization on direct access, not only listing visibility.
[ ] Verify object/tenant ownership in business logic.
[ ] Keep Host/Origin and SSRF protections enabled/configured.
[ ] Use exact public URLs for OAuth discovery/redirects.
[ ] Separate inbound caller credentials from upstream credentials.
[ ] Constrain OpenAPI/proxy origins and redirects.
[ ] Sandboxed model code never receives server secrets unnecessarily.
[ ] File access is rooted, canonicalized, size-limited, tenant-isolated.
[ ] Approval UX is not treated as authorization.
[ ] Identity-dependent cache entries are partitioned.
[ ] Persistent session/task stores have ACL/TTL/encryption policy.
[ ] Error responses do not leak internals.
[ ] Logs are auditable and redacted.
[ ] Rate/concurrency/result/task quotas are defined.
[ ] FastMCP/security dependencies are pinned and monitored.
```

### Sources

1. https://gofastmcp.com/servers/auth/authentication
2. https://gofastmcp.com/servers/authorization
3. https://gofastmcp.com/servers/visibility
4. https://gofastmcp.com/servers/middleware
5. https://gofastmcp.com/deployment/http
6. https://gofastmcp.com/apps/providers/approval
7. https://gofastmcp.com/apps/providers/file-upload
8. https://gofastmcp.com/servers/transforms/code-mode
9. https://github.com/PrefectHQ/fastmcp/security/advisories

# FastMCP Advanced — 33) Performance, scaling, resilience, and large-catalog engineering
### Performance, scaling, resilience, and large-catalog engineering

### 33.0 Performance model

FastMCP overhead can come from several distinct places:

```text
startup/import
  + component discovery/schema generation
  + provider listing/remote calls
  + middleware/auth
  + transport/serialization
  + tool execution
  + result serialization
  + task/state/cache storage
  + LLM token cost of the published catalog
```

Do not optimize “FastMCP” as one black box. Measure which layer dominates.

---

## 33.1 Startup and import cost

Large servers can spend meaningful time importing:

* SDK clients;
* database libraries;
* ML models;
* hundreds of component modules;
* auth-provider dependencies;
* Prefab/app packages.

Mitigation:

```text
cheap module import
  -> FastMCP construction
      -> shared expensive initialization in lifespan
```

Do not connect to databases or load large models at module import unless the launcher lifecycle truly requires it. Use lifespan so startup/teardown is explicit and testable.

---

## 33.2 `fastmcp-slim` for narrower dependency footprints

FastMCP 3.3 split the importable implementation into `fastmcp-slim`, while the `fastmcp` distribution remains the normal full package. Client-framework authors can use `fastmcp-slim[client]` to avoid pulling in unnecessary server/CLI surface.

Use the full `fastmcp` package by default for ordinary application servers. Use slim packaging when dependency footprint itself is a design constraint and your tests prove the selected extras cover the runtime surface.

---

## 33.3 Sync tool execution and threadpool dispatch

Normal synchronous tools are dispatched so they do not block the event loop.

```python
@mcp.tool
def cpu_light_library_call(x: int) -> int:
    return sync_library(x)
```

For thread-affine libraries FastMCP 3.3 added `run_in_thread=False`:

```python
@mcp.tool(run_in_thread=False)
def thread_affine_operation() -> str:
    return call_library_bound_to_event_loop_thread()
```

This runs inline on the event-loop thread.

Tradeoff:

```text
run_in_thread=True/default -> avoids event-loop blocking, but changes thread
run_in_thread=False        -> preserves thread affinity, blocks every other request while running
```

Use inline sync mode only for short thread-affine operations. It is not a general performance optimization.

### Timeout caveat

Inline sync execution has no cooperative cancellation checkpoints. Do not combine `run_in_thread=False` with assumptions that a FastMCP foreground timeout can interrupt a long blocking sync call.

---

## 33.4 Async tools for I/O concurrency

Prefer `async def` when using async-native clients:

```python
@mcp.tool
async def fetch_customer(customer_id: str) -> dict:
    return await db.fetch_customer(customer_id)
```

Avoid wrapping an async client in synchronous blocking adapters just to keep a `def` signature. Conversely, do not convert CPU-bound pure Python work to `async def` and expect it to become parallel.

For heavy CPU work:

* move to a process pool/native extension/service;
* or run as a background task with appropriate worker resources.

---

## 33.5 Tool catalog token cost

A large tool catalog creates two costs:

1. server/client listing and schema serialization;
2. LLM context/token/selection overhead.

Once catalogs reach hundreds or thousands of tools, the second cost often dominates user experience.

FastMCP 3.1's Tool Search transforms exist specifically for this case.

---

## 33.6 Tool Search

Conceptual architecture:

```text
1000 real tools
  -> search transform
      model initially sees:
        search_tools(...)
        execute_tool(...)
        perhaps schema/details helper
```

The model discovers only candidates relevant to the request rather than ingesting every full schema.

Use when:

* OpenAPI-generated catalogs are large;
* many mounted/remote providers are aggregated;
* tool names/descriptions are searchable;
* model tool selection degrades with raw catalog size.

Do not use search to compensate for poor names/descriptions; search quality depends on the metadata being meaningful.

---

## 33.7 Code Mode

Code Mode goes further by letting the model write small orchestration code that searches/loads/executes components through a sandbox.

Value:

* fewer model round-trips for multi-tool workflows;
* dynamic composition of many safe tools;
* reduces up-front schema injection.

Costs:

* sandbox execution overhead;
* more complex security posture;
* need to test orchestration/failure semantics;
* harder observability if code and tool traces are not correlated.

Use when the workload benefits from programmatic composition—not merely because the catalog is large.

---

## 33.8 Pagination

`list_page_size` controls protocol list pagination. Use it when raw lists can be large even if a search transform is not used.

Tradeoff:

```text
small page -> less per-response memory/token, more round trips
large page -> fewer round trips, larger response and client work
```

Choose based on client behavior and catalog size. Do not set an arbitrary tiny page size without testing clients that automatically enumerate the full catalog anyway.

---

## 33.9 Provider latency

Provider listing can require filesystem/remote/OpenAPI/custom work. Avoid providers that perform expensive full scans on every `list_tools()` without caching/indexing.

Provider optimization strategies:

* precompute stable metadata at startup;
* incremental refresh/index;
* memoize immutable schema descriptions;
* use provider-local caches with clear invalidation;
* set timeouts on remote provider calls;
* isolate slow providers behind gateways/search.

Do not cache principal-dependent provider results globally unless the cache is identity-partitioned.

---

## 33.10 Remote proxy fan-out

A gateway mounting several remote MCP servers can turn one list/call into multiple network operations.

Design for:

```text
per-upstream timeout
circuit breaker / failure isolation
clear namespace
bounded retries
partial catalog policy
health status
```

FastMCP 3.4 changed proxy initialization to fail loudly rather than silently masking an unavailable upstream. Treat that as a feature: startup/readiness should reveal a dependency failure unless your architecture explicitly supports degraded mode.

---

## 33.11 Client connection reuse

A programmatic `Client` owns a session/transport lifecycle. Avoid recreating a remote client for every small operation when a shared initialized session is safe and useful.

```python
async with Client(url) as client:
    # multiple calls share one initialized lifecycle
    ...
```

Use `client.new()` when you need an independent session with the same configuration.

Connection reuse is not universally correct: if authentication principal changes, construct a distinct client/session/cache partition.

---

## 33.12 Stateful vs stateless HTTP scaling

Stateful Streamable HTTP keeps server-side session semantics. It can complicate horizontal scaling because consecutive requests must reach compatible session state.

Stateless HTTP removes that affinity requirement and is usually better for multi-worker/load-balanced deployments when the application does not rely on stateful MCP-session features.

Decision:

| Requirement | Prefer |
| --- | --- |
| elicitation/sampling/session-coupled v3 features | stateful HTTP |
| simple stateless tool API at high scale | stateless HTTP |
| durable app/user state across replicas | explicit shared store, not process memory |

Do not assume cookie sticky sessions solve MCP affinity reliably for every client.

---

## 33.13 Shared state/store latency

Persistent `session_state_store`, task backends, caches, and event stores can add a network round trip to each operation.

Optimization rules:

* store coarse-grained state, not every trivial local variable;
* batch writes where safe;
* choose Redis/Valkey/database locality near workers;
* use TTLs;
* avoid huge serialized blobs;
* separate hot task metadata from bulk artifacts.

Measure backend p95/p99 separately from tool execution.

---

## 33.14 Background tasks and worker scaling

Use tasks when work is long enough that keeping an interactive request open is undesirable.

Scaling model:

```text
MCP frontend replicas
  -> shared Docket/task backend
      -> N workers
          -> shared result/progress backend
```

For multi-process durability, use a shared backend rather than the default in-memory process-local configuration.

Capacity dimensions:

* task queue depth;
* worker concurrency;
* per-task memory/CPU;
* external API quotas;
* cancellation latency;
* result retention.

---

## 33.15 Middleware overhead

Every middleware layer wraps many or all operations. Avoid expensive work in a broad `on_message` hook if it is only needed for tool calls.

Prefer the narrowest hook:

```text
all messages -> request -> call_tool -> specific component
```

Move expensive logging serialization off the critical path when possible. Sample high-volume traces rather than logging full payloads on every request.

---

## 33.16 Caching

Cache only where semantics permit.

Good candidates:

* immutable public resource;
* expensive deterministic metadata lookup;
* public read-only API response with explicit freshness window.

Bad default candidates:

* session state;
* authorization-sensitive response;
* personalized resource;
* destructive mutation result;
* rapidly changing operational data.

A fast wrong/cross-user response is worse than an uncached correct response.

---

## 33.17 Schema dereferencing cost

FastMCP normally dereferences JSON Schema `$ref` structures for client compatibility. Deep/large Pydantic schemas can become verbose.

Mitigation:

* keep MCP-facing models narrow;
* avoid exposing giant internal domain models directly;
* use facade request/response models;
* consider whether every nested field belongs in the tool contract.

Do not disable dereferencing purely for size unless every target client is verified to handle referenced schemas correctly.

---

## 33.18 Result size and streaming

MCP tool results are not a substitute for a bulk data plane. For very large datasets:

* return a resource URI/object reference;
* stream/export to object storage;
* paginate;
* summarize + provide drill-down tools;
* avoid multi-megabyte JSON tool results by default.

The LLM often needs a decision-relevant subset, not the full raw dataset.

---

## 33.19 HTTP reverse-proxy buffering

Streamable HTTP responses may rely on streaming/SSE behavior under the hood. Reverse proxies can destroy latency characteristics by buffering.

Production proxy policy commonly includes:

```text
proxy buffering off for MCP streaming route
long enough read/send timeouts
HTTP keepalive
request/body size limits
TLS termination
```

Test time-to-first-event and long-running response behavior through the actual load balancer.

---

## 33.20 Observability for performance work

Measure:

```text
startup seconds
component-list latency
provider-list latency
schema bytes / tool count
call p50/p95/p99
middleware/auth time
upstream HTTP time
serialization bytes/time
task queue wait + run time
state/cache backend time
open connections / workers
error/timeout/cancel rate
```

Use OpenTelemetry spans plus application metrics. Do not infer bottlenecks from end-to-end duration only.

---

## 33.21 Large-catalog architecture pattern

```text
remote/generated providers
  -> provider-local namespace/curation
  -> authorization-safe server aggregation
  -> ToolSearch or CodeMode
  -> small discovery surface to model
  -> execute selected component
```

This pattern keeps the source catalog broad without forcing the model to ingest it all.

---

## 33.22 Performance anti-patterns

* 2,000 raw model-visible tools with no search strategy.
* Running a blocking 5-second sync function with `run_in_thread=False`.
* Reconnecting a remote client for every operation.
* Full provider rescans on every tool listing.
* Global caching of user-dependent results.
* Process-local state in a horizontally scaled deployment.
* One frontend process executing hour-long background work inline.
* Huge internal Pydantic models exposed directly as tool schemas.
* Multi-megabyte tool results sent to the model instead of a resource/export path.
* Benchmarking direct localhost while production proxy buffers streams.

---

## 33.23 Agent checklist

```text
[ ] Measure startup, catalog, call, provider, state, and task latency separately.
[ ] Use lifespan for shared expensive initialization.
[ ] Prefer async for async I/O.
[ ] Use run_in_thread=False only for short thread-affine sync work.
[ ] Introduce ToolSearch/Code Mode for large catalogs deliberately.
[ ] Use pagination where raw listing remains large.
[ ] Cache provider metadata safely and with invalidation.
[ ] Give remote upstreams timeouts/failure isolation.
[ ] Reuse Client sessions when principal/session semantics allow.
[ ] Choose stateful vs stateless HTTP intentionally.
[ ] Use shared durable backends for scaled tasks/state.
[ ] Keep middleware hooks narrow.
[ ] Bound result/schema sizes.
[ ] Test streaming through the real reverse proxy.
[ ] Instrument with OTEL + service metrics.
```

### Sources

1. https://gofastmcp.com/servers/transforms/tool-search
2. https://gofastmcp.com/servers/transforms/code-mode
3. https://gofastmcp.com/servers/pagination
4. https://gofastmcp.com/servers/tools
5. https://gofastmcp.com/clients/client
6. https://gofastmcp.com/deployment/http
7. https://gofastmcp.com/servers/tasks
8. https://gofastmcp.com/servers/telemetry
9. https://gofastmcp.com/getting-started/installation

# FastMCP Advanced — 34) Production architecture patterns
### Production architecture patterns

### 34.0 Pattern catalog

FastMCP is flexible enough that “how do I deploy it?” has no single answer. The useful unit is a production pattern with explicit answers for:

```text
transport
process ownership
state lifetime
auth boundary
provider topology
catalog strategy
long-running work
observability
failure mode
```

The following patterns are reference architectures, not mandatory templates.

---

## 34.1 Pattern A — local desktop/IDE STDIO server

```text
MCP host (Claude/Cursor/etc.)
  -> spawns Python process
      -> FastMCP STDIO
          -> local tools/resources
```

Use for:

* developer tools;
* local filesystem/repository access;
* single-user desktop workflows;
* tools that should inherit the user's local OS permissions.

Recommended:

```python
if __name__ == "__main__":
    mcp.run()  # STDIO
```

Controls:

* explicit dependency environment (`uv`/install config);
* no stdout logging;
* minimal local filesystem scope;
* local secret handling through environment/OS keychain;
* avoid remote-server auth assumptions.

Failure mode: host cannot launch/import the subprocess because its environment differs from your shell.

---

## 34.2 Pattern B — internal shared Streamable HTTP service

```text
multiple MCP clients
  -> internal HTTPS/load balancer
      -> FastMCP HTTP service
          -> DB/internal APIs
```

Use for shared team services where centralized deployment and common data access matter.

Recommended:

* `mcp.http_app()` under Uvicorn/ASGI;
* authentication even on “internal” networks when data is sensitive;
* stateless HTTP if session features are unnecessary and horizontal scaling is desired;
* shared DB/service pools in lifespan;
* OTEL and request metrics;
* rate/concurrency limits.

---

## 34.3 Pattern C — authenticated enterprise multi-tenant service

```text
MCP client
 -> OAuth/OIDC
    -> edge / reverse proxy
       -> FastMCP auth + authorization
          -> tenant-aware providers/services
             -> enterprise systems
```

Required design decisions:

```text
principal identity key
 tenant id source
 per-tenant authorization
 state/cache partition
 upstream credential strategy
 audit schema
 data retention
```

Avoid one shared `SessionContext`-style mutable global containing tenant secrets. In FastMCP terms, use injected current principal/token plus tenant-aware service dependencies and persistent stores keyed by trusted identity.

---

## 34.4 Pattern D — remote MCP gateway / aggregator

```text
client
 -> one FastMCP gateway
      + local tools
      + remote MCP provider A
      + remote MCP provider B
      + OpenAPI provider C
        -> namespaces/transforms
        -> auth/visibility
        -> search transform
```

Use when clients should integrate once while the platform team manages multiple sources.

Controls:

* namespace every source deliberately;
* fail-loud/readiness policy for upstream initialization;
* timeout/circuit-break each upstream;
* do not forward caller credentials blindly;
* authorize the final published surface;
* ToolSearch/Code Mode for very large aggregate catalogs;
* distributed traces across gateway/upstream calls.

---

## 34.5 Pattern E — generated OpenAPI adapter behind curated facade

```text
OpenAPIProvider (low-level generated operations)
  -> hidden/namespaced
  -> hand-authored workflow tools
      -> model-visible business API
```

Use when an existing REST API is broad/implementation-oriented.

Benefits:

* fast initial integration;
* direct access remains available for controlled maintenance/debug;
* model sees stable business workflows;
* REST refactors do not necessarily break MCP contract.

The facade should own transaction semantics, object authorization, and error normalization.

---

## 34.6 Pattern F — large catalog with Tool Search

```text
hundreds/thousands of safe tools
  -> Search transform
      model sees search + execute surface
```

Use when token/tool-selection cost is the problem and workflows remain naturally discrete tool calls.

Requirements:

* high-quality names/descriptions/tags;
* deterministic search tests;
* authorization before indexing/execution;
* schema-fetch/details flow if needed.

Do not keep both the giant raw catalog and the search meta-tools model-visible unless there is a clear reason.

---

## 34.7 Pattern G — Code Mode orchestration platform

```text
curated safe tool universe
  -> CodeMode sandbox
      -> model writes orchestration code
          -> search/schema/execute components
```

Use when a model frequently needs to combine many small operations and explicit tool-call round trips become inefficient.

Required:

* sandbox with resource/network/filesystem limits;
* effectful backend auth;
* code/tool trace correlation;
* execution time/memory limits;
* deterministic fallback for sandbox errors.

Do not expose raw server secrets/environment to the sandbox.

---

## 34.8 Pattern H — long-running task service

```text
MCP frontend
  -> submit task
      -> shared queue/backend
          -> task workers
              -> progress/result store
```

Use for:

* report generation;
* ETL/indexing;
* long external jobs;
* batch analysis;
* model/document processing.

Production:

* `fastmcp[tasks]` in v3;
* Redis/Valkey/shared Docket backend;
* worker process lifecycle separate from web frontend;
* result TTL;
* cancellation policy;
* task ownership authorization;
* progress throttling.

---

## 34.9 Pattern I — interactive Apps server

```text
MCP host with Apps support
 -> model calls app entry tool
    -> renderer iframe
       -> app-only backend tools
          -> server services/state
```

Use for dashboards, forms, file upload, approvals, or human-in-the-loop work surfaces.

Production:

* pin Prefab;
* app/model visibility discipline;
* CSP/permissions for custom HTML;
* real host acceptance tests;
* persistent file/state scope for stateless/scaled deployments;
* server-side enforcement behind approval UI.

---

## 34.10 Pattern J — hybrid FastAPI + MCP service

```text
one ASGI process
  /api/* -> FastAPI REST
  /mcp/* -> FastMCP http_app
```

Use when the same service/product exposes both conventional HTTP APIs and MCP.

Required:

* compose/forward lifespans correctly;
* keep REST auth and MCP auth semantics explicit;
* share pools through application/lifespan state carefully;
* avoid path/base-URL confusion;
* route `.well-known` correctly for OAuth;
* independently rate-limit REST and MCP workloads if needed.

---

## 34.11 Pattern K — serverless/stateless MCP

Use only if your required MCP features tolerate stateless requests.

```text
client
 -> stateless HTTP request
    -> ephemeral worker
       -> shared DB/object/state backends
```

Good fit:

* short independent tools;
* shared durable business state already externalized;
* no dependence on process-local session objects.

Poor fit:

* v3 interactive features requiring persistent session callback semantics;
* in-memory FileUpload scope;
* process-local task backends;
* expensive per-request startup.

---

## 34.12 Pattern L — high-security brokered service

```text
public client
 -> OAuth resource server
    -> policy gateway
       -> narrow FastMCP catalog
          -> credential broker
             -> protected upstream systems
```

Use when upstream systems must never receive raw client credentials or when every action needs centrally audited policy.

Controls:

* strong issuer/audience verification;
* short-lived scoped upstream credentials;
* allowlisted tools/actions;
* object-level authorization;
* immutable audit trail;
* error masking;
* no arbitrary proxy/OpenAPI origins;
* egress network policy.

---

## 34.13 Context/state ownership by architecture

| Pattern | Process-local state acceptable? | Shared store needed? |
| --- | ---: | ---: |
| local STDIO | often | only for durability |
| single-instance HTTP | sometimes | for restart durability |
| multi-worker stateless HTTP | no for cross-request features | yes |
| task service | no for durable/distributed tasks | yes |
| multi-tenant SaaS | only request-local | yes for session/business state |
| FileUpload in stateless HTTP | no | durable/user-keyed file store |

---

## 34.14 Auth ownership by architecture

```text
Local STDIO:
  OS/user/launcher boundary

Remote HTTP:
  FastMCP auth + authorization

Gateway:
  inbound auth != outbound upstream credential

Hybrid FastAPI:
  REST auth and MCP auth may share IdP but are separate request surfaces
```

Document which layer validates token issuer/audience and which code decides component/object permissions.

---

## 34.15 Deployment readiness checklist

```text
[ ] Exact FastMCP version pinned.
[ ] Python/dependency lockfile reproducible.
[ ] Server name/version/instructions explicit.
[ ] Transport and public path documented.
[ ] Lifespan initializes/tears down all shared resources.
[ ] Auth/authorization tests cover direct access.
[ ] Host/Origin/proxy/TLS settings tested in production topology.
[ ] `.well-known` OAuth routing correct if applicable.
[ ] Catalog size strategy selected.
[ ] Persistent state/task/cache backend selected where needed.
[ ] Result/file/upload size limits configured.
[ ] Reverse proxy does not buffer streaming unexpectedly.
[ ] OTEL/logging/metrics include request/tool/task identifiers.
[ ] Contract manifest/fingerprints stored.
[ ] Rollback version and data/schema compatibility known.
```

---

## 34.16 Failure-mode planning

For every external dependency, define behavior:

| Dependency | Fail-closed? | Degraded mode? |
| --- | --- | --- |
| auth/JWKS | normally fail closed | cached validated keys only under defined policy |
| remote provider | often readiness failure | optional provider may be omitted if product allows |
| DB | tool failure / readiness | read-only cache if semantically safe |
| task backend | reject new long tasks | foreground fallback only if bounded/safe |
| telemetry | should not break business path | buffer/drop according to policy |
| Apps renderer CDN/assets | plain response or explicit app error | depends on product UX |

Do not let an observability outage become an authorization bypass or a remote-provider outage silently expose stale/incorrect capabilities.

---

## 34.17 Anti-pattern inventory

* One architecture accidentally serving local STDIO, public HTTP, tasks, Apps, and multi-tenancy without explicit boundaries.
* Process-memory session/file/task state behind multiple workers.
* Gateway forwarding inbound bearer tokens directly to every upstream.
* Large OpenAPI catalog published raw to the model.
* Long tasks running inside web request workers.
* Hybrid FastAPI mounting without lifespan propagation.
* Multi-tenant cache without tenant partitioning.
* Production Apps with floating Prefab dependency.
* Serverless deployment of session-dependent v3 workflows.

---

## 34.18 Agent architecture questionnaire

```text
1. Who launches/owns the server process?
2. Which transport(s) are required?
3. Is the server local-user or remote multi-user?
4. Which auth principal and tenant model applies?
5. Which components/providers are public?
6. How large is the catalog?
7. Does the workflow require session callbacks or Apps?
8. Can requests be stateless?
9. Which state must survive process restart?
10. Which jobs exceed normal request duration?
11. Which upstream credentials are used and how are they derived?
12. What are the file/network/code sandbox boundaries?
13. How are contract changes detected?
14. What is the failure/degraded-mode policy for each dependency?
15. What telemetry proves correctness and performance?
```

### Sources

1. https://gofastmcp.com/deployment/running-server
2. https://gofastmcp.com/deployment/http
3. https://gofastmcp.com/servers/composition
4. https://gofastmcp.com/servers/providers/overview
5. https://gofastmcp.com/servers/transforms/tool-search
6. https://gofastmcp.com/servers/transforms/code-mode
7. https://gofastmcp.com/servers/tasks
8. https://gofastmcp.com/apps/overview
9. https://gofastmcp.com/integrations/fastapi

# FastMCP Advanced — 35) API stability, upgrade discipline, and FastMCP 2 → 3 migration
### API stability, upgrade discipline, and FastMCP 2 → 3 migration

### 35.0 Versioning stance

FastMCP tracks a quickly evolving protocol ecosystem. For production, treat a FastMCP upgrade as an API/protocol dependency upgrade, not as a routine utility-library bump.

This reference is pinned to stable **3.4.7**. Recommended dependency policy:

```toml
[project]
dependencies = ["fastmcp==3.4.7"]
```

Then upgrade intentionally with contract/security tests and the official upgrade/changelog pages.

---

## 35.1 Why exact pinning matters

An MCP framework can change:

* protocol behavior;
* generated schemas;
* component metadata;
* auth defaults;
* transport/security policy;
* client result models;
* task/session semantics;
* OpenAPI conversion;
* CLI behavior.

Even a source-compatible upgrade can alter what an LLM sees and therefore change model routing behavior. Store the FastMCP version next to MCP contract snapshots.

---

## 35.2 v2 -> v3 architectural change summary

FastMCP 3's fundamental change is the provider/transform architecture plus a cleaner split between server definition and transport execution.

```text
FastMCP 2 mental model:
  server object + local registries + constructor transport knobs + older transforms/imports

FastMCP 3 mental model:
  FastMCP aggregate server
    -> LocalProvider + additional Providers
    -> provider transforms
    -> server transforms
    -> visibility/auth/session policy
  transport bound later by run/http_app/CLI
```

---

## 35.3 Removed v2 constructor transport kwargs

In v3, do not write:

```python
# v2-style: stale
mcp = FastMCP("server", host="0.0.0.0", port=8080)
```

Write:

```python
mcp = FastMCP("server")

if __name__ == "__main__":
    mcp.run(transport="http", host="0.0.0.0", port=8080)
```

Transport/deployment settings such as host, port, HTTP paths, JSON response/stateless mode belong at serve time, not server-definition construction.

---

## 35.4 Duplicate policy consolidation

Older separate duplicate settings for tools/resources/prompts were consolidated into one `on_duplicate=` policy.

Migration principle:

```text
component-type-specific constructor knobs
  -> unified aggregate-server behavior
```

Prefer `on_duplicate="error"` in strict development/CI when accidental collisions should never pass silently.

---

## 35.5 Tag filtering -> visibility transforms

Old include/exclude tag constructor configuration is replaced by server-level visibility operations:

```python
mcp.enable(tags={"public"}, only=True)
mcp.disable(tags={"internal"})
```

Visibility now composes with provider/server/session transforms, which is more powerful but means ordering should be tested explicitly.

---

## 35.6 Tool serializer -> explicit `ToolResult`

Custom serialization should move into the tool's response contract:

```python
from fastmcp.tools.tool import ToolResult

@mcp.tool
def operation() -> ToolResult:
    return ToolResult(
        content="Human summary",
        structured_content={"ok": True},
    )
```

This keeps custom result behavior local to the tool instead of a global serializer knob.

---

## 35.7 Component list APIs

Old `get_tools()` / `get_resources()` / `get_prompts()`-style collection APIs were replaced by `list_*` APIs. Expect sequences/lists rather than old dict-shaped assumptions.

Audit generated/utility code for patterns like:

```python
for name, tool in mcp.get_tools().items():
    ...
```

and migrate to the current async listing/resolution APIs.

---

## 35.8 Decorators return original functions in v3

A major behavioral change:

```python
@mcp.tool
def add(a: int, b: int) -> int:
    return a + b

# `add` remains the normal Python function.
assert add(2, 3) == 5
```

Do not expect `add.name`, `add.description`, or other component-model attributes on the decorated function. Resolve the actual component from the server when you need metadata:

```python
tool = await mcp.get_tool("add")
```

This is beneficial for testing and ordinary Python reuse but breaks code that relied on decorator return objects.

---

## 35.9 Method registration: standalone decorators

For methods, use standalone `@tool()`, `@resource()`, or `@prompt()` to attach metadata, then register the **bound method**.

```python
from fastmcp.tools import tool

class Calculator:
    @tool()
    def multiply(self, x: int) -> int:
        return x * 2

calc = Calculator()
mcp.add_tool(calc.multiply)
```

This prevents `self`/`cls` from appearing in the MCP schema.

---

## 35.10 `Context` state became async

Session-state methods in v3 must be awaited:

```python
value = await ctx.get_state("key")
await ctx.set_state("key", value)
await ctx.delete_state("key")
```

Audit helper functions where missing `await` can compile but return coroutine objects into business logic.

---

## 35.11 Prompt message type migration

Use FastMCP `Message` rather than older low-level prompt-message constructions/raw dict coercion.

```python
from fastmcp.prompts import Message

@mcp.prompt
def review() -> list[Message]:
    return [Message("Review the following code.")]
```

This makes the prompt render contract explicit and type-safe.

---

## 35.12 Auth provider configuration

v3 auth providers no longer rely on the same implicit environment-variable auto-loading patterns as older code. Pass provider configuration explicitly, typically reading secrets through `os.environ`/your secret manager at application construction.

Avoid hidden global environment magic in reusable libraries; explicit config is easier to audit and test.

---

## 35.13 WebSocket transport removal

Legacy WebSocket transport support is removed. Use Streamable HTTP for new remote/network deployments.

If old client config refers to a WS transport, treat the migration as a transport-contract change and test the host/client configuration—not just server Python source.

---

## 35.14 `mount()` over import/copy semantics

`import_server(...)` is deprecated in v3 in favor of live `mount(...)` composition.

These are not identical concepts:

```text
import/copy -> snapshot of components
mount       -> live nested provider relationship; child lifecycle/middleware can matter
```

When migrating, test behavior instead of performing a blind name replacement.

---

## 35.15 Proxy construction

Prefer current provider/proxy APIs (`create_proxy(...)`, `ProxyProvider`) rather than deprecated older module paths or `FastMCP.as_proxy(...)` patterns.

This aligns remote composition with the general Provider architecture.

---

## 35.16 Tool transformations

Older transformation helper APIs move toward explicit Transform objects such as `ToolTransform` plus server/provider transform stacks.

Migration rule:

```text
imperative one-off transformation mutation
  -> declarative Transform object in ordered pipeline
```

Audit ordering because later transforms can override earlier publication changes.

---

## 35.17 Metadata namespace change

FastMCP-specific metadata uses the `fastmcp` namespace rather than older `_fastmcp` conventions.

If custom middleware/provider logic reads raw metadata keys, test it directly against the v3 component model rather than assuming old private-key names remain compatible.

---

## 35.18 Background tasks become an optional dependency

In v3, task support is opt-in through the tasks extra.

```bash
uv add 'fastmcp[tasks]==3.4.7'
```

A project that upgrades without the extra can import ordinary FastMCP but fail when task-specific classes/features are used. Add dependency tests to CI for the actual extras your server requires.

---

## 35.19 3.x milestone map

| Version family | High-level additions relevant to this reference |
| --- | --- |
| 3.0 | Providers/transforms architecture, versioning, session state/visibility, tasks, telemetry, major CLI/config changes |
| 3.1 | Tool Search, Code Mode, MultiAuth and additional auth capabilities |
| 3.2 | Apps release: `FastMCPApp`, built-in app providers, Generative UI |
| 3.3 | `fastmcp-slim`, client-only packaging, `run_in_thread=False`, auth/OTEL hardening |
| 3.4 | `fastmcp-remote`, proxy fail-loud behavior, HTTP/OAuth/SSRF/Host-Origin hardening, final 3.4.7 fixes |

Use the changelog/release page for exact patch-level details when upgrading.

---

## 35.20 Upgrade workflow

```text
1. Pin current production version.
2. Read official upgrade guide + all changelog entries to target.
3. Update package in a branch.
4. Run import/type/test suite.
5. Generate MCP manifest/fingerprints.
6. Review schema/description/catalog drift.
7. Run auth/security regression suite.
8. Run STDIO/HTTP/App/task integration tests as applicable.
9. Deploy staging behind real proxy/IdP.
10. Compare telemetry and host acceptance.
11. Roll out with rollback artifact retained.
```

---

## 35.21 Automated migration grep list

Search older code for:

```text
FastMCP(... host=
FastMCP(... port=
include_tags
exclude_tags
tool_serializer
on_duplicate_tools
on_duplicate_resources
on_duplicate_prompts
get_tools(
get_resources(
get_prompts(
PromptMessage
import_server(
as_proxy(
add_tool_transformation
_fastmcp
FASTMCP_DECORATOR_MODE
ctx.get_state( without await
ctx.set_state( without await
```

Review each hit against the official upgrade guide; do not perform unreviewed global substitutions.

---

## 35.22 Upgrade tests that matter most

```text
[ ] server imports with exact production extras
[ ] decorators remain callable as Python functions where expected
[ ] component catalogs match baseline
[ ] transport args moved to run/http_app
[ ] mounted child lifecycle/middleware behavior correct
[ ] visibility rules still hide/enable intended components
[ ] auth providers initialize with explicit secrets/config
[ ] session state persists/is isolated as expected
[ ] prompts render typed Message objects
[ ] tasks work with fastmcp[tasks]
[ ] generated OpenAPI provider calls same upstream request shape
```

---

## 35.23 Anti-pattern inventory

* `pip install -U fastmcp` directly in production with no pin/contract diff.
* Copying v2 code examples into a v3 project.
* Blind `import_server` -> `mount` replacement without lifecycle tests.
* Assuming a decorated function is a Tool object.
* Leaving transport settings in `FastMCP(...)` constructor.
* Treating deprecation warnings as harmless indefinitely.
* Upgrading auth/proxy behavior without replaying security tests.
* Reading current main-branch v4 docs and applying beta-only APIs to stable 3.4.7.

---

## 35.24 Agent checklist

```text
[ ] Pin stable FastMCP exactly.
[ ] Read the migration guide before editing code.
[ ] Separate constructor/server-definition changes from transport changes.
[ ] Replace old filtering with visibility/transform APIs.
[ ] Treat decorators as original functions, resolve component metadata from server.
[ ] Use bound-method standalone decorators where needed.
[ ] Await v3 session-state methods.
[ ] Use FastMCP Message/PromptResult contracts.
[ ] Add task extra if task features are used.
[ ] Migrate imports/proxies/transforms to provider architecture.
[ ] Compare protocol manifests/fingerprints before and after upgrade.
[ ] Run security/host/deployment acceptance tests.
```

### Sources

1. https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2
2. https://gofastmcp.com/changelog
3. https://gofastmcp.com/updates
4. https://gofastmcp.com/development/releases
5. https://gofastmcp.com/servers/providers/overview
6. https://gofastmcp.com/servers/transforms/transforms

# FastMCP Advanced — 36) FastMCP 4 prerelease transition guide
### FastMCP 4 prerelease transition guide

### 36.0 Read this section as migration research, not stable API guidance

As of the date of this reference, the deployable baseline used throughout Sections 0–35 is **FastMCP 3.4.7**. FastMCP **4.0.0b3** is a prerelease. Do not mix code from this section into a 3.4.7 project unless the item is explicitly described as a migration action.

Use this section to answer:

```text
Should we stay on 3.x for now?
What breaks when we test v4?
Which architectural assumptions change because MCP itself changed?
```

---

## 36.1 Why v4 is different from a normal FastMCP major bump

FastMCP 4 rebuilds on the MCP Python SDK v2 and the modern 2026-era protocol. The largest changes are not cosmetic FastMCP refactors; they follow changes in MCP itself:

* modern sessionless request semantics;
* protocol-era negotiation;
* server callback capabilities change;
* background tasks move to an extension;
* Python protocol model fields move toward snake_case attributes;
* HTTP stack moves to `httpx2`;
* new session/identity/extension primitives become available.

Migration therefore needs architecture review, not only import fixes.

---

## 36.2 Prerelease installation isolation

Do not replace production 3.4.7 in-place to “see what happens.” Create a branch/lockfile/environment dedicated to v4 beta testing.

Official prerelease shape:

```bash
pip install 'fastmcp==4.0.0b3'
```

With `uv`, the prerelease relationship between `fastmcp` and `fastmcp-slim` may require explicit prerelease constraints as documented by the v4 upgrade guide.

Keep production and beta lockfiles separate until migration is approved.

---

## 36.3 Python protocol model fields: snake_case in code, camelCase on wire

MCP Python SDK v2 renames Python model attributes from protocol camelCase to Pythonic snake_case.

Examples:

```text
inputSchema       -> input_schema
outputSchema      -> output_schema
structuredContent -> structured_content
isError           -> is_error
mimeType          -> mime_type
nextCursor        -> next_cursor
serverInfo        -> server_info
```

The wire JSON remains protocol-compatible camelCase through aliases. This is a **Python attribute migration**, not a wire-format redesign.

FastMCP 4 provides compatibility shims for many reads during the transition, but new code should use the current snake_case attributes.

Audit any code that manually inspects MCP models, middleware payloads, or manifests.

---

## 36.4 Modern sessionless protocol

The modern protocol moves away from assuming a long-lived server-held session connection through which the server can call back into the client mid-request.

This has direct impact on three v3 patterns:

```text
server-initiated sampling
server-initiated roots access
server-initiated elicitation during an open request
```

Do not assume these v3 `Context` capabilities survive unchanged.

---

## 36.5 Server-initiated sampling and roots are removed on modern v4 connections

The v4 upgrade guide explicitly calls out removal of server-side context methods such as:

```text
ctx.sample(...)
ctx.sample_step(...)
ctx.list_roots(...)
```

The **client side** can still provide sampling/roots capabilities in contexts where a client API owns them; the removed assumption is a server relying on callback behavior during a modern request.

Architecture decision:

* If borrowing the **caller's current model** is fundamental to the product, remaining on FastMCP 3.x may be appropriate until you have a redesigned interaction.
* If the server can own its own model provider credentials, replace callback sampling with an explicit provider SDK/service dependency.
* If roots were only convenience metadata, make required paths/resources explicit inputs or use a different durable resource model.

---

## 36.6 Elicitation becomes multi-round `InputRequiredResult`

In the modern protocol, a tool does not hold the original request open while synchronously asking the client for more input. Instead:

```text
call tool
  -> tool returns InputRequiredResult describing required input
      -> request completes
          -> client obtains user input
              -> client calls tool again with input responses/request state
                  -> tool resumes from the top and returns next input request or final result
```

Representative v4 shape from the upgrade/current docs uses `mcp.types.InputRequiredResult` plus request-state security and `ctx.input_responses`.

This is an architectural rewrite for tools that use `ctx.elicit(...)` in v3.

### Migration design rule

Refactor interactive tools into a replayable state machine:

```python
@mcp.tool
async def interactive_operation(ctx: Context, ...) -> FinalResult | InputRequiredResult:
    if ctx.input_responses is None:
        return make_input_request(...)

    # validate supplied response and either ask again or finish
    ...
```

Keep all durable/action state idempotent because the tool function may execute from the top on each round.

---

## 36.7 Request-state security

Multi-round input requires the server to protect the opaque state carried between requests. The v4 docs introduce request-state signing/security keys for this reason.

Scaled deployment rule:

```text
all replicas validating/resuming the same workflow
  -> must share the same request-state security key set
```

Keys must be high-entropy secret material and rotated through an intentional key-set strategy. Do not place workflow secrets directly into unsigned client-carried state.

---

## 36.8 `UserSession` and `SessionId`

V4 introduces explicit application session primitives that do not rely on a persistent transport connection.

Concept:

```text
UserSession -> server-side state bucket keyed by authenticated user
SessionId   -> explicit client-visible handle when one user needs multiple independent buckets
```

Representative modern API:

```python
from fastmcp import FastMCP
from fastmcp.server.sessions import UserSession

mcp = FastMCP("Assistant")

@mcp.tool
async def remember(fact: str, session: UserSession) -> str:
    facts = await session.get("facts", default=[])
    facts.append(fact)
    await session.set("facts", facts)
    return f"Remembered {len(facts)} facts"
```

`UserSession` depends on authentication because the authenticated principal determines the bucket.

For multi-replica durability, configure shared `session_state_store`; process-local memory remains process-local.

---

## 36.9 Background tasks move to the tasks extension

V3 task syntax like `@mcp.tool(task=True)` remains conceptually useful, but v4's protocol support is extension-based. The current v4 docs show adding `TasksExtension` from the optional `fastmcp-tasks` package.

Conceptual migration:

```python
from fastmcp_tasks import TasksExtension

mcp.add_extension(TasksExtension())

@mcp.tool(task=True)
async def long_job(...):
    ...
```

Audit:

* task-enabled resources/prompts—v4 modern task support is tool-focused;
* client calls that passed explicit `task=True` flags under old semantics;
* direct imports from old task modules;
* Docket/worker injection APIs changed/removed from core surfaces.

Use the v4 tasks docs for the exact beta version you test.

---

## 36.10 General server extensions

FastMCP 4 exposes an extension system where a server/client can advertise negotiated capabilities under extension identifiers and attach handlers/interceptors/lifespan behavior.

This is the architectural foundation for task support and future protocol capabilities.

Platform implication:

```text
core MCP protocol
  + negotiated extensions
      -> product-specific capability surface
```

Do not expose an extension-dependent workflow without verifying client support/negotiation.

---

## 36.11 Argument completion

V4 can answer MCP completion requests for prompt/resource-template arguments. A completion function can consider:

* the current argument;
* partial user text;
* previously provided argument values.

Use for IDE-like discoverability of IDs, environments, dataset names, branches, or resource paths. Treat completion output as suggestions, not authorization; final component execution must still validate/authorize the supplied value.

---

## 36.12 `httpx` -> `httpx2`

FastMCP 4 changes its HTTP stack to `httpx2`.

Audit code where you directly integrate with FastMCP transports/auth/OpenAPI:

```text
httpx.AsyncClient passed to FastMCP transport/provider
httpx.Auth subclass passed to Client/OAuth
except httpx.ConnectError around FastMCP operations
httpx_client_factory callbacks
```

The subtle failure is exception handling: if ordinary `httpx` remains installed transitively, `except httpx.ConnectError` still imports and type-checks but will not catch an `httpx2.ConnectError` raised by the v4 FastMCP stack.

HTTP calls **inside your own business tool** can continue using whichever HTTP library you choose; the migration specifically concerns objects/exceptions interacting with FastMCP's HTTP implementation.

---

## 36.13 Dependency-floor changes

The current v4 upgrade guide calls out higher dependency floors including Pydantic and the server web stack. Projects pinned to older FastAPI/Starlette combinations may face resolver conflicts before any server code runs.

Migration process:

```text
1. inspect current pydantic/FastAPI/Starlette pins
2. resolve documented v4 floors in a branch
3. run REST/FastAPI integration tests too
4. only then diagnose FastMCP runtime differences
```

Do not interpret an unsatisfiable resolver as an MCP runtime bug.

---

## 36.14 Deprecated v3 surfaces removed in v4

The v4 upgrade guide enumerates many aliases/deprecations that existed in 3.x but are removed. High-value audit signals include:

```text
FastMCP.as_proxy(...)
import_server(...)
mount(prefix=...)
mount(as_proxy=...)
add_tool_transformation(...)
remove_tool_transformation(...)
legacy proxy/openapi import paths
FASTMCP_DECORATOR_MODE
old tool serializer/exclude_args parameters
old sampling handler constructor parameters
old task/sampling module imports
```

Do not infer the replacement from the name alone; several replacements change lifecycle or error semantics.

---

## 36.15 `import_server` migration deserves special attention

In v3, `import_server` already headed toward deprecation. In v4 it is removed.

Replacement `mount()` is **live composition**, not a static copy:

```text
old import snapshot
  !=
mounted provider with child lifecycle/middleware
```

If your old code depended on snapshot behavior or intentionally skipped child lifespan/middleware, redesign rather than search/replace.

---

## 36.16 Middleware/session assumptions

Audit middleware that assumes:

* an initialization hook always establishes persistent connection state;
* `ctx.set_state` on one request is guaranteed to be visible on the next modern connection;
* every message is a routable request;
* request-scoped objects live for an entire interactive workflow.

V4 pushes application state toward explicit durable session/request-state primitives instead of transport-session assumptions.

---

## 36.17 OAuth issuer/base URL behavior

The v4 upgrade guide notes behavior changes around OAuth issuer/base URL consistency. Any OAuth deployment with a custom issuer distinct from the resource base URL deserves a full reauthorization/discovery test.

Do not migrate auth in production without testing:

```text
.well-known authorization-server metadata
protected-resource metadata
authorize/token redirect chain
audience/issuer claims
existing client reauthorization behavior
```

---

## 36.18 Protocol-era negotiation

During transition, clients/servers may support multiple protocol eras. Treat negotiated protocol version/capabilities as a runtime input.

Do not branch business correctness on guessed client brand names. Branch on negotiated capability/protocol features where the API exposes them.

This is especially important for elicitation/sampling behaviors that differ fundamentally across protocol eras.

---

## 36.19 Stay on 3.x vs migrate to v4

### Strong reasons to remain on 3.x temporarily

* product fundamentally requires server-initiated sampling through the caller's model;
* extensive `ctx.elicit()` workflows not yet refactored to multi-round tools;
* production clients have not validated modern protocol behavior;
* migration dependency floors conflict with a larger application stack;
* prerelease policy forbids beta dependencies.

### Strong reasons to test v4 early

* planning a new long-lived service architecture;
* need modern sessionless scale semantics;
* want explicit user sessions/extensions/completions;
* need to validate future client ecosystem direction;
* existing v3 server is stateless/simple and likely to migrate cleanly.

Testing v4 early does not require deploying it early.

---

## 36.20 Migration branch workflow

```text
[ ] Freeze current 3.4.7 manifest/fingerprints.
[ ] Create separate v4 prerelease dependency lock.
[ ] Resolve dependency floors.
[ ] Run official v3->v4 grep/audit list.
[ ] Fix imports/removed APIs.
[ ] Replace FastMCP-facing httpx objects/exceptions with httpx2.
[ ] Refactor ctx.elicit workflows to InputRequiredResult state machines.
[ ] Decide what to do with server-initiated sampling/roots.
[ ] Add TasksExtension for task tools.
[ ] Move cross-request state to UserSession/SessionId/shared store where appropriate.
[ ] Audit middleware/session assumptions.
[ ] Re-run contract fingerprints.
[ ] Run auth/proxy/security suite.
[ ] Run every target MCP host against negotiated modern protocol.
[ ] Only then consider production rollout.
```

---

## 36.21 V4 migration anti-patterns

* Copying beta code into stable 3.4.7 modules piecemeal.
* Treating sessionless protocol as “HTTP stateless flag, but otherwise same semantics.”
* Replacing `ctx.elicit()` with arbitrary prompt text and assuming equivalent UX.
* Keeping `except httpx.*` around FastMCP v4 calls.
* Enabling task tools without the v4 tasks extension.
* Assuming `mount()` exactly reproduces removed `import_server()` behavior.
* Ignoring dependency-floor conflicts and force-installing incompatible packages.
* Testing only with in-memory client while production hosts negotiate different protocol eras.
* Migrating a sampling-dependent product without deciding who owns the model call afterward.

---

## 36.22 V4 agent rule

```text
Sections 0–35 = stable FastMCP 3.4.7 implementation reference.
Section 36     = prerelease migration guide.

When generating deployable code:
  - use stable sections unless project explicitly pins v4 beta;
  - if project pins v4, re-read current v4 docs because beta APIs can change;
  - never silently mix major-version examples.
```

### Sources

1. https://gofastmcp.com/getting-started/upgrading/from-fastmcp-3
2. https://github.com/PrefectHQ/fastmcp/releases
3. https://pypi.org/project/fastmcp/
4. https://gofastmcp.com/servers/sessions
5. https://gofastmcp.com/servers/elicitation
6. https://gofastmcp.com/servers/extensions
7. https://gofastmcp.com/servers/completions
8. https://gofastmcp.com/servers/tasks

# FastMCP Advanced — 37) Dense appendices and lookup matrices
### Dense appendices and lookup matrices

These appendices are lookup-first. They compress the public FastMCP surface into matrices and rule tables so syntax retrieval is faster than rereading the narrative chapters. The tables below are distilled from the current FastMCP docs and SDK reference surfaces. ([FastMCP][1])

---

## A) Decorator-argument matrix

The matrix below condenses the common/public decorator surfaces for tools, resources, and prompts, plus the cross-cutting task and auth capabilities documented elsewhere. Resource standalone-decorator docs also expose `title`/`icons` on the lower-level surface; prompt docs expose `title` directly; task docs explicitly extend `task=` to tools/resources/resource-templates/prompts; authorization docs explicitly allow component-level auth on tools/resources/prompts. ([FastMCP][1])

| Argument / behavior            |                                                   `@mcp.tool(...)` |               `@mcp.resource(...)` | `@mcp.prompt(...)` | Lookup note                                                                |
| ------------------------------ | -----------------------------------------------------------------: | ---------------------------------: | -----------------: | -------------------------------------------------------------------------- |
| required positional identifier |                                                                  — |                     `uri` required |                  — | Resources/templates require a URI/template string                          |
| `name=`                        |                                                                  ✓ |                                  ✓ |                  ✓ | Exposed MCP name; defaults to Python function name                         |
| `description=`                 |                                                                  ✓ |                                  ✓ |                  ✓ | Overrides docstring-derived description                                    |
| `title=`                       | (not primary public docs surface; use annotations/title semantics) | lower-level/standalone SDK surface |                  ✓ | Human-facing title                                                         |
| `mime_type=`                   |                                                                  — |                                  ✓ |                  — | Resource MIME contract                                                     |
| `tags=`                        |                                                                  ✓ |                                  ✓ |                  ✓ | Grouping / filtering / policy tagging                                      |
| `icons=`                       |                                                                  ✓ |                                  ✓ |                  ✓ | Client-facing icon metadata                                                |
| `annotations=`                 |                                                                  ✓ |                                  ✓ |                  — | Tool/resource advisory hints                                               |
| `meta=`                        |                                                                  ✓ |                                  ✓ |                  ✓ | Static definition metadata                                                 |
| `version=`                     |                                                                  ✓ |                                  ✓ |                  ✓ | Component versioning key                                                   |
| `output_schema=`               |                                                                  ✓ |                                  — |                  — | Tool structured-output schema only                                         |
| `timeout=`                     |                                                                  ✓ |                                  — |                  — | Foreground execution timeout                                               |
| `task=`                        |                                                                  ✓ |                                  ✓ |                  ✓ | Also applies to resource templates; async-only for task-enabled components |
| `auth=`                        |                                                                  ✓ |                                  ✓ |                  ✓ | Component-level authorization checks                                       |
| `enabled=`                     |                                                         deprecated |         older/compat surfaces only |         deprecated | Prefer server-level visibility controls                                    |

### Auto-inference rules

FastMCP’s auto-inference contract is intentionally asymmetric across component types. Tools infer executable schema from parameter and return annotations; resources infer read shape from URI + return type; prompts infer argument schema from signature but still send arguments as strings over MCP. ([FastMCP][1])

| Auto-inferred item            | Tool                                               | Resource                                           | Prompt                          |
| ----------------------------- | -------------------------------------------------- | -------------------------------------------------- | ------------------------------- |
| default public name           | function name                                      | function name (plus URI remains canonical address) | function name                   |
| default description           | docstring                                          | docstring                                          | docstring                       |
| input schema source           | function signature + type hints                    | URI/template + function params                     | function signature + type hints |
| return/render contract source | return annotation / `output_schema` / `ToolResult` | return value / `ResourceResult`                    | return value / `PromptResult`   |

---

## B) Dependency-injection matrix

FastMCP DI is request-scoped, hidden from MCP schemas, recursively resolved, and cached per request across the dependency tree. Shared subdependencies are resolved once per request; nested dependencies are supported; async context-manager dependencies are cleaned up after the component call completes. ([FastMCP][2])

| Primitive              | Injects                                        | Scope                                  | Hidden from MCP schema | Availability / failure mode                                   | Best use                                                                |
| ---------------------- | ---------------------------------------------- | -------------------------------------- | ---------------------: | ------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `Context` annotation   | FastMCP `Context`                              | request-scoped                         |                      ✓ | Only valid during request execution                           | Logging, progress, resource/prompt access, sampling, elicitation, state |
| `CurrentContext()`     | same as above, explicit DI form                | request-scoped                         |                      ✓ | Same as above                                                 | Preferred explicit context injection                                    |
| `Depends(fn)`          | return value of sync/async/async-CM dependency | request-scoped                         |                      ✓ | Dependency exceptions propagate                               | Config, DB sessions, service objects, request-derived values            |
| `CurrentFastMCP()`     | active `FastMCP` server                        | request/task-scoped                    |                      ✓ | Raises outside server context                                 | Introspection, server-owned facilities                                  |
| `CurrentRequest()`     | Starlette `Request`                            | HTTP request-scoped                    |                      ✓ | Raises outside HTTP contexts                                  | Headers, client IP, HTTP-specific behavior                              |
| `CurrentAccessToken()` | validated `AccessToken`                        | authenticated HTTP request/task-scoped |                      ✓ | Raises when unauthenticated                                   | Claims, scopes, user/tenant identity                                    |
| `Progress()`           | progress reporter                              | immediate + background execution       |                      ✓ | Uses Docket progress in workers; in-memory fallback otherwise | Long-running ops, task progress                                         |
| `CurrentDocket()`      | active Docket instance                         | task-scoped                            |                      ✓ | Requires `fastmcp[tasks]`; raises outside task/server context | Scheduling additional tasks, advanced task control                      |
| `CurrentWorker()`      | active Docket worker                           | task-scoped                            |                      ✓ | Requires `fastmcp[tasks]`; raises outside worker context      | Worker metadata, concurrency-aware logic                                |

### DI rules worth memorizing

| Rule                                                  | Operational meaning                                                             |
| ----------------------------------------------------- | ------------------------------------------------------------------------------- |
| Dependency params are excluded from the public schema | Clients never see runtime-only values as callable args                          |
| Nested dependencies are supported                     | Build services from smaller deps instead of giant edge functions                |
| Cache is per-request                                  | Same dependency instance is reused within one request only                      |
| Context managers are valid dependencies               | Use them for per-request resources that need cleanup                            |
| Sensitive/runtime-only values belong in DI            | Never expose tokens, requests, DB handles, or worker objects as public MCP args |

---

## C) Task-mode truth table

The table below reflects the documented task execution modes and poll-interval behavior from the background-task docs. Background tasks require the `tasks` optional extra, task-enabled components must be async, and `task=True` is shorthand for `TaskConfig(mode="optional")`. The documented default poll interval is 5 seconds unless overridden with `TaskConfig(..., poll_interval=...)`. ([FastMCP][3])

| `TaskConfig.mode` | Client call **without** task augmentation | Client call **with** task augmentation | Boolean shorthand      | Operational use                |
| ----------------- | ----------------------------------------- | -------------------------------------- | ---------------------- | ------------------------------ |
| `"forbidden"`     | Executes synchronously                    | Error: task not supported              | `task=False` / omitted | Hard no-background contract    |
| `"optional"`      | Executes synchronously                    | Executes as background task            | `task=True`            | Dual-mode compatibility        |
| `"required"`      | Error: task required                      | Executes as background task            | none                   | Hard async/background contract |

### Task execution quick rules

| Rule                                                    | Consequence                                                  |
| ------------------------------------------------------- | ------------------------------------------------------------ |
| task-enabled component must be `async def`              | sync + `task=True` is invalid                                |
| `timeout=` is foreground-only                           | background time limits must be handled by Docket/task policy |
| `Progress()` works in immediate and background modes    | same code path can surface progress either way               |
| dynamic late-added components are not worker-registered | define task-enabled components before workers start          |

---

## D) Provider comparison grid

Providers are the component-source layer. Local code definitions land in `LocalProvider`; mounted child servers become wrapped providers; remote backends use proxy providers; filesystem and skills providers source components/resources from external layout conventions. Provider order matters: local provider first, then added providers in registration order. ([FastMCP][4])

| Provider primitive                          | Source model                                          | Best use                                                    | Strength                                             | Primary caveat                                                                    |
| ------------------------------------------- | ----------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------- |
| `LocalProvider`                             | code-defined components on the server itself          | ordinary single-server definitions                          | simplest, highest precedence                         | not a composition mechanism by itself                                             |
| `FastMCPProvider` / `mount(child)`          | live in-process child FastMCP server                  | modular monorepos, feature servers                          | child middleware/execution semantics preserved       | namespacing strongly recommended                                                  |
| `ProxyProvider` / `create_proxy(...)`       | remote or external MCP server via client factory      | gateways, transport bridging, backend aggregation           | forwards rich MCP interactions                       | proxied components have `task_config.mode="forbidden"`; catalog caching may stale |
| `FileSystemProvider`                        | scan Python files for standalone-decorated components | convention-based local plugin/component discovery           | no direct server-instance coupling required in files | `reload=True` is dev-oriented and rescans on each request                         |
| `SkillProvider` / `SkillsDirectoryProvider` | skill directories exposed as resources                | agent-skill corpora, skill packs, prompt/resource libraries | natural `skill://...` resource model                 | choose `supporting_files="template"` vs `"resources"` deliberately                |
| Custom `Provider`                           | arbitrary source: DB, API, registry, tenant catalog   | non-code component sources                                  | exact control over sourcing + provider lifespan      | author must own correctness/performance/caching                                   |

### Provider-side rules

| Rule                                                        | Consequence                                                             |
| ----------------------------------------------------------- | ----------------------------------------------------------------------- |
| local provider is first                                     | local overrides are the safest way to shadow mounted/proxied components |
| provider order matters                                      | first matching provider wins                                            |
| providers source components; they do not replace middleware | request policy still belongs to middleware/auth/visibility layers       |
| use provider `lifespan()` for source-owned connections      | open DB/API clients once at server startup                              |

---

## E) Transform comparison grid

Transforms are the component-rewrite layer. Provider-level transforms run first and only affect that source; server-level transforms run after provider aggregation and therefore shape the final client-visible catalog. Later transforms wrap earlier ones. ([FastMCP][4])

| Transform                                                         | Layer              | What it rewrites                                                    | Best use                                            | Key caveat                                     |
| ----------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------- |
| `Namespace("x")`                                                  | provider or server | prefixes tool/prompt names and resource/template URIs               | collision avoidance under composition               | changes client-visible names/URIs              |
| `ToolTransform(...)`                                              | provider or server | tool name, description, tags, schema, argument contract             | adapt mounted/proxied tools without copying         | later transforms see already-transformed names |
| `ToolSearch(...)` (`RegexSearchTransform`, `BM25SearchTransform`) | server or provider | replaces full tool listing with `search_tools` + `call_tool` façade | very large tool catalogs                            | clients must search to discover hidden tools   |
| `ResourcesAsTools(server)`                                        | server only        | exposes resource listing/reading as synthetic tools                 | tool-only clients that cannot use resource protocol | apply to a server, not a raw provider          |
| `PromptsAsTools(server)`                                          | server only        | exposes prompt listing/rendering as synthetic tools                 | tool-only clients that cannot use prompt protocol   | apply to a server, not a raw provider          |

### Transform placement rules

| Placement      | Sees                                 | Best for                                               |
| -------------- | ------------------------------------ | ------------------------------------------------------ |
| provider-level | one source only; runs first          | local namespacing or per-source schema cleanup         |
| server-level   | fully aggregated catalog; runs later | global publication policy, final naming, global search |

### `add_transform(...)` vs `wrap_transform(...)`

| Method                | Effect                           | Use when                                            |
| --------------------- | -------------------------------- | --------------------------------------------------- |
| `add_transform(...)`  | mutates provider/server in place | single-owner mutable composition                    |
| `wrap_transform(...)` | returns a new provider wrapper   | reusing one provider in multiple parents/namespaces |

---

## F) Client transport decision matrix

The transport and client docs make the deployment recommendation explicit: use Streamable HTTP for new production HTTP deployments, SSE only for backward compatibility, explicit `env=...` for STDIO, and in-memory for most tests. ([FastMCP][5])

| Transport                       | Choose when                                      | Lifecycle owner              |     Auth helpers relevant? | Key caveat                                                                   |
| ------------------------------- | ------------------------------------------------ | ---------------------------- | -------------------------: | ---------------------------------------------------------------------------- |
| In-memory (`Client(server)`)    | tests, local harnesses, same-process integration | client + same process        |                          — | not a real deployment transport                                              |
| `StdioTransport`                | local subprocess/server launched by client       | client manages subprocess    |                          — | **must** pass `env=...` explicitly; shell env is not inherited automatically |
| `StreamableHttpTransport`       | new production HTTP deployments                  | remote server manages itself |                          ✓ | configure TLS/auth/headers explicitly                                        |
| `SSETransport`                  | older/legacy HTTP integrations                   | remote server manages itself |                          ✓ | backward compatibility only; not the default for new deployments             |
| config / multi-server transport | one client surface over multiple servers         | mixed                        | depends on child transport | namespacing complexity shifts to client side                                 |

---

## G) Client auth decision matrix

Client auth helpers are only meaningful on HTTP transports. Bearer auth is the non-interactive token path; OAuth is the interactive browser path; CIMD is an OAuth enhancement for verifiable client identity when the server supports it. ([FastMCP][6])

| Client auth form                 | Use when                                                                   | Transport scope | User interaction | Key caveat                                                              |
| -------------------------------- | -------------------------------------------------------------------------- | --------------: | ---------------: | ----------------------------------------------------------------------- |
| `auth="<token>"`                 | you already have a bearer token                                            |        HTTP/SSE |               no | FastMCP adds `Bearer ` automatically; do not include it yourself        |
| `BearerAuth(token=...)`          | same as above, but explicit auth object                                    |        HTTP/SSE |               no | same bearer semantics as raw string                                     |
| `auth="oauth"`                   | simplest interactive OAuth flow                                            |        HTTP/SSE |              yes | browser/callback required; default in-memory token storage is ephemeral |
| `OAuth(...)`                     | scopes, persistent storage, pre-registered clients, advanced OAuth control |        HTTP/SSE |              yes | use encrypted token storage in production                               |
| `OAuth(client_metadata_url=...)` | CIMD: verifiable domain-backed client identity                             |        HTTP/SSE |              yes | requires server support and public HTTPS metadata document              |

---

## H) Output-schema and content-block conversion rules

The matrix below condenses the tool/resource/prompt return rules and the client-side read paths. Tool output schemas are object-shaped; primitive tool outputs are wrapped under a `"result"` key when structured output is required; `ToolResult` gives explicit control; media helpers auto-convert when returned directly or in a list. On the client side, tool calls expose `.data` / `.structured_content` / `.content`, resource reads return content items with `.text` / `.blob`, and prompt renders return `messages`. ([FastMCP][1])

### H.1 Server-side conversion matrix

| Server return shape                                              |                       Structured output? |          Traditional content blocks? | Client-side primary read path                           | Rule                                                        |                              |
| ---------------------------------------------------------------- | ---------------------------------------: | -----------------------------------: | ------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------- |
| tool primitive with return annotation (`-> int`, `-> str`, etc.) |                                        ✓ |                                    ✓ | `result.data` (primitive) or `result.content`           | server wraps under `"result"` for schema compliance         |                              |
| tool primitive without return annotation / without schema        |                               usually no |                                    ✓ | `result.content`                                        | `.data` may be `None`                                       |                              |
| tool `dict` / dataclass / Pydantic model                         |                                        ✓ |                                    ✓ | `result.data` hydrated, `result.structured_content` raw | object-like returns naturally become structured content     |                              |
| tool with explicit `output_schema={...}`                         |                                        ✓ |                                    ✓ | `result.data` / `result.structured_content`             | schema root must be object                                  |                              |
| tool returning `ToolResult`                                      |                     exactly what you set |                 exactly what you set | whatever fields you emitted                             | bypasses most automatic wrapping                            |                              |
| tool returning `Image` / `Audio` / `File` directly or in list    | not by itself unless wrapped/objectified |              ✓ media/resource blocks | `result.content`                                        | nested inside dict/object requires manual `.to_*_content()` |                              |
| resource returning `str`                                         |                       resource text item |                                  n/a | `content[i].text`                                       | MIME defaults/overrides apply                               |                              |
| resource returning `bytes`                                       |                                blob item |                                  n/a | `content[i].blob`                                       | set MIME type explicitly when needed                        |                              |
| resource returning `ResourceResult`                              |                     exactly what you set |                                  n/a | content items + metadata                                | full manual control for resources                           |                              |
| prompt returning `str`                                           |                                      n/a |         rendered as one user message | `result.messages`                                       | single user message                                         |                              |
| prompt returning `list[Message                                   |                                    str]` |                                  n/a | rendered message sequence                               | `result.messages`                                           | strings become user messages |
| prompt returning `PromptResult`                                  |                                      n/a | rendered message sequence + metadata | `result.messages`                                       | explicit render-time control                                |                              |

### H.2 Client-side result-object cheat sheet

| Operation            | High-level return             | Most important fields                                   |                                |
| -------------------- | ----------------------------- | ------------------------------------------------------- | ------------------------------ |
| `call_tool(...)`     | `CallToolResult`              | `.data`, `.structured_content`, `.content`, `.is_error` |                                |
| `read_resource(...)` | list of `TextResourceContents | BlobResourceContents`                                   | `.text` / `.blob`, `.mimeType` |
| `get_prompt(...)`    | `GetPromptResult`             | `.messages`, plus prompt description metadata           |                                |

---

## I) Deployment mounting invariants

The HTTP deployment docs are explicit about several mount-time invariants that are easy to get wrong. Treat the table below as the hard checklist for Starlette/FastAPI composition and mounted OAuth-protected servers. ([FastMCP][5])

| Invariant                                 | Correct rule                                                                                                                | If wrong                                                            |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Parent lifespan wiring                    | pass `lifespan=mcp_app.lifespan` to the parent app, or combine lifespans explicitly                                         | session manager will not initialize correctly                       |
| Mount prefix + internal MCP path          | final endpoint = mount prefix + `http_app(path=...)` path                                                                   | accidental double-prefixing or unexpected endpoint paths            |
| “I want `/mcp` exactly”                   | use `http_app(path="/")` and mount at `"/mcp"`                                                                              | `http_app(path="/mcp")` + mount `"/mcp"` yields `/mcp/mcp`          |
| Nested mounts                             | prefixes append recursively                                                                                                 | final endpoint differs from what you expect if you forget one layer |
| Mounted OAuth `.well-known`               | discovery routes stay at the **domain root**, not under the mount prefix                                                    | RFC-compliant discovery breaks                                      |
| `base_url` vs `mcp_path` on mounted OAuth | `base_url` includes the external mount prefix; `mcp_path` is only the internal MCP path; actual URL = `base_url + mcp_path` | doubled prefixes like `/api/api/mcp`                                |

### Fast path examples

| Desired external URL | Correct pattern                                                 |
| -------------------- | --------------------------------------------------------------- |
| `/analytics/mcp/`    | mount at `/analytics`, use `http_app(path="/mcp")`              |
| `/mcp`               | mount at `/mcp`, use `http_app(path="/")`                       |
| `/outer/inner/mcp/`  | nested mounts `/outer` → `/inner`, with `http_app(path="/mcp")` |

---

## J) Fast lookup rules

These are the shortest operational heuristics directly implied by the docs above. ([FastMCP][5])

| Need                                                       | Reach for                                                    |
| ---------------------------------------------------------- | ------------------------------------------------------------ |
| hide runtime-only values from MCP schema                   | DI (`Depends`, `CurrentContext`, `CurrentAccessToken`, etc.) |
| long-running work with client-visible progress/status      | `task=True` / `TaskConfig(...)` + `Progress()`               |
| combine child servers safely                               | `mount(child, namespace="...")`                              |
| expose remote/external servers under one parent            | `create_proxy(...)` / `ProxyProvider`                        |
| avoid catalog collisions                                   | `Namespace(...)`                                             |
| adapt mounted/proxied tool schema without rewriting source | `ToolTransform(...)`                                         |
| huge tool catalogs hurting token budget                    | `ToolSearch(...)`                                            |
| tool-only client needs resources/prompts                   | `ResourcesAsTools(server)` / `PromptsAsTools(server)`        |
| local reproducible server contract                         | `fastmcp.json`                                               |
| exact client-visible manifest verification                 | `fastmcp inspect --format mcp`                               |
| local app preview without a host client                    | `fastmcp dev apps ...`                                       |

This appendix is the lookup layer; it should be the fastest path to “what knob do I need?” and “what surface should I use?”

[1]: https://gofastmcp.com/servers/tools "Tools"
[2]: https://gofastmcp.com/servers/dependency-injection "Dependency Injection"
[3]: https://gofastmcp.com/servers/tasks "Background Tasks"
[4]: https://gofastmcp.com/servers/providers/overview "Providers"
[5]: https://gofastmcp.com/deployment/http "HTTP Deployment"
[6]: https://gofastmcp.com/changelog "Changelog"

---

## L) Stable version-gate matrix

This table is intentionally coarse-grained. Use the changelog for patch-level details and the relevant chapter for API semantics.

| Capability | Stable introduction / major milestone | Stable-reference note |
| --- | --- | --- |
| Provider/Transform architecture | 3.0 | Core v3 composition model |
| Component versioning | 3.0 | Tools/resources/prompts can coexist by version |
| Session state + per-session visibility | 3.0 | v3 request/session model; v4 redesign is separate |
| Background task framework | 3.0 | Requires tasks extra in v3 |
| OpenTelemetry integration | 3.0 | Use for server/client/runtime observability |
| Tool Search | 3.1 | Large-catalog token/selection optimization |
| Code Mode | 3.1 | Sandbox-based model orchestration over catalog |
| `MultiAuth` | 3.1 | Multiple auth mechanisms/providers |
| FastMCP Apps release | 3.2 | `FastMCPApp`, built-in app providers, Generative UI |
| `fastmcp-slim` distribution split | 3.3 | Importable core package / client-only footprint |
| `run_in_thread=False` | 3.3 | Thread-affine sync tools; blocks event loop |
| `fastmcp-remote` | 3.4 | Remote HTTP -> STDIO bridge use case |
| Proxy initialization fail-loud behavior | 3.4 | Upstream initialization errors surface intentionally |
| HTTP/OAuth/SSRF Host-Origin hardening | 3.4.x | Keep current patched stable version |
| Stable target of this document | **3.4.7** | Released August 10, 2026 |
| FastMCP 4 beta | **4.0.0b3** at document date | Prerelease; see Section 36 only |

---

## M) Package / extra selection matrix

| Need | Dependency stance | Notes |
| --- | --- | --- |
| Normal server + CLI development | `fastmcp==3.4.7` | Default recommendation |
| Background tasks | `fastmcp[tasks]==3.4.7` | Docket/task dependencies |
| Apps / Prefab | `fastmcp[apps]==3.4.7` plus pinned tested `prefab-ui` | Pin UI dependency explicitly |
| Code Mode | `fastmcp[code-mode]==3.4.7` | Sandbox/runtime dependencies |
| LLM-provider helpers if used | corresponding FastMCP extra (`openai`, `anthropic`, `gemini`, etc.) | Only install what project uses |
| Lightweight client-framework dependency | `fastmcp-slim[client]==3.4.7` | Avoid full server/CLI dependency surface |
| STDIO bridge to remote server | `fastmcp-remote` / documented remote bridge package | Use for host transport adaptation, not programmatic client code |
| V4 beta experiment | exact beta in isolated environment | Never float prerelease into stable lockfile |

Python floor for stable 3.4.7 is **Python 3.10+**. Record the actual interpreter in `fastmcp.json`, `pyproject.toml`, and CI where reproducibility matters.

---

## N) Server-definition vs deployment matrix

| Concern | Put it on / in | Do not put it in |
| --- | --- | --- |
| Server name/instructions/version | `FastMCP(...)` | reverse-proxy config |
| providers/transforms/middleware/auth | `FastMCP(...)` or post-construction add methods | shell-only launcher script when topology is code-owned |
| host/port/path/transport | `run(...)`, `run_async(...)`, `http_app(...)`, CLI/config | v3 `FastMCP(...)` constructor |
| ASGI workers/process management | Uvicorn/Gunicorn/platform | tool decorators |
| public mount prefix | parent ASGI/router/proxy + OAuth `base_url` contract | duplicated into internal `mcp_path` |
| state durability | `session_state_store`, task/event stores, business DB | process globals in scaled deployment |
| dependency environment | `pyproject.toml` / lockfile / `fastmcp.json` | implicit developer shell state |

---

## O) Transport decision matrix

| Transport/deployment | Use when | Strong guidance |
| --- | --- | --- |
| STDIO | local desktop/editor/CLI; client owns process | default local path; no arbitrary stdout logs |
| Streamable HTTP | remote/shared/multi-client network service | default new remote transport |
| SSE | legacy client compatibility | do not choose for new design |
| In-memory `Client(mcp)` | tests / deterministic in-process orchestration | not a production network boundary |
| `fastmcp-remote` bridge | host can launch STDIO but actual server is HTTP | transport adapter, not gateway composition |
| ProxyProvider/create_proxy | republish upstream MCP through a new server | treat upstream as a security/availability dependency |

---

## P) HTTP deployment invariants

```text
[ ] mcp.run() is guarded by if __name__ == "__main__" when launched as a script.
[ ] Use mcp.http_app() when ASGI process/middleware/mounting control is needed.
[ ] Mounted Starlette/FastAPI app forwards/combines mcp_app.lifespan.
[ ] mount_prefix + internal MCP path yields exactly one public path.
[ ] OAuth base_url includes external mount prefix.
[ ] OAuth mcp_path is only the internal MCP path.
[ ] base_url + mcp_path equals the actual public MCP URL.
[ ] .well-known routes remain at standards-required root paths.
[ ] Reverse proxy streaming buffering is disabled for MCP stream route.
[ ] Host/Origin trusted-proxy policy is tested through real ingress.
[ ] stateless_http is chosen intentionally for scaled deployments.
[ ] process-local session/file/task state is not relied upon across replicas.
```

---

## Q) Tool authoring quick matrix

| Requirement | Preferred FastMCP technique |
| --- | --- |
| normal action | `@mcp.tool` |
| bound instance method | standalone `@tool()` + `mcp.add_tool(instance.method)` |
| hidden DB/service dependency | `Depends(...)` |
| request/client runtime capabilities | `CurrentContext()` / `Context` |
| current access token | `CurrentAccessToken()` |
| exact input constraints | `Annotated[..., Field(...)]` |
| object structured output | typed dict/dataclass/Pydantic model return |
| exact response control | `ToolResult(...)` |
| media/file direct return | `Image` / `Audio` / `File` helper |
| foreground timeout | `@mcp.tool(timeout=...)` |
| long-duration work | task mode + durable backend |
| thread-affine sync library | `run_in_thread=False` with short execution only |
| multiple compatible implementations | same name + explicit `version=` |
| component-specific auth | `auth=...` |

Never expose `*args` / `**kwargs` in a tool contract. Build a stable typed wrapper around dynamic internal APIs.

---

## R) Tool-result conversion quick matrix

| Python return | MCP effect |
| --- | --- |
| `str` | text content |
| `bytes` | binary/embedded resource-style content |
| `dict` / dataclass / Pydantic | content + structured object representation |
| primitive with annotated output schema | object-wrapped structured result as needed for MCP schema |
| `Image` / `Audio` / `File` directly or list item | matching MCP media/resource block |
| existing MCP content block | passed through |
| `ToolResult` | explicit control over content / structured content / runtime metadata |
| `None` | empty response |

Media helpers nested inside arbitrary dicts are not automatically transformed in the same way as direct returns; convert them explicitly when nesting.

---

## S) Resource selection matrix

| Data shape | Preferred resource form |
| --- | --- |
| computed fixed URI | `@mcp.resource("scheme://fixed")` |
| parameterized family | URI template via `@mcp.resource("scheme://{id}")` |
| literal in-memory text | `TextResource` |
| literal bytes | `BinaryResource` |
| local file | `FileResource` |
| remote HTTP content | `HttpResource` |
| directory listing | `DirectoryResource` |
| multi-item response / custom metadata | `ResourceResult` + `ResourceContent` |

Template rule:

```text
required function argument -> path template parameter
optional client-configurable argument -> {?query} parameter
optional hidden fixed default -> omit from URI template
```

---

## T) Prompt quick matrix

| Need | Preferred form |
| --- | --- |
| one user instruction | return `str` |
| conversation seed | return `list[Message | str]` |
| dynamic description/meta | `PromptResult` |
| method prompt | standalone `@prompt()` + bound registration |
| runtime context | injected `Context` |
| evolving contract | same prompt name + `version=` |

Prompt arguments are strings at the MCP protocol boundary even when FastMCP converts them into typed Python values. Keep complex prompt-argument structures modest and well-described.

---

## U) Context capability matrix — stable v3

| Context capability | V3 use | V4 note |
| --- | --- | --- |
| logging | `ctx.debug/info/warning/error` | core concept remains |
| progress | `ctx.report_progress(...)` | task/extension semantics evolve |
| read/list resources | supported | verify v4 current API |
| list/render prompts | supported | verify v4 current API |
| elicitation | `ctx.elicit(...)` | modern v4 path uses multi-round `InputRequiredResult` |
| sampling | `ctx.sample(...)`, `sample_step(...)` | removed from modern server-side v4 context |
| roots | server can access client roots in v3 | server callback path removed in modern v4 |
| session state | `get_state/set_state/delete_state` | v4 favors explicit `UserSession`/`SessionId` |
| per-session visibility | enable/disable/reset | revalidate against v4 protocol/session model |
| lifespan context | `ctx.lifespan_context` | shared startup state remains a core pattern |

---

## V) Provider / transform decision matrix

| Requirement | Use |
| --- | --- |
| local decorator-defined components | built-in `LocalProvider` (automatic) |
| mount child FastMCP server | `mount(...)` / FastMCP provider relationship |
| remote MCP source | `ProxyProvider` / proxy server |
| filesystem components | `FileSystemProvider` |
| agent skills | skills provider where applicable in stable v3 |
| OpenAPI-generated components | `OpenAPIProvider` |
| source-local rename/filter | provider-level Transform |
| global final publication policy | server-level Transform |
| avoid component name collision | `Namespace(...)` |
| reshape tool names/descriptions/args | `ToolTransform` |
| expose resources to tool-only clients | `ResourcesAsTools` |
| expose prompts to tool-only clients | `PromptsAsTools` |
| hide giant catalog behind discovery | Tool Search transform |
| model-programmable orchestration | Code Mode |

Provider order is semantic. LocalProvider comes first; do not rely on accidental name collisions when namespaces provide an explicit design.

---

## W) Visibility / version / authorization matrix

```text
Provider source
 -> provider transforms
 -> server transforms
 -> final visibility / auth / session policy
 -> protocol listing
 -> direct execution still enforces auth
```

| Mechanism | Purpose | Security boundary? |
| --- | --- | ---: |
| Namespace | collision-free naming | no |
| Visibility enable/disable | publication/discovery policy | no, by itself |
| Versioning | compatible evolution / implementation selection | no |
| Tool Search | reduce catalog exposed to LLM | no |
| Component `auth=` | authorization | yes, for defined check |
| object-level business check | row/object/tenant permission | yes |
| Approval app | user-facing advisory UX | no |

---

## X) Authentication decision matrix

| Identity environment | Prefer |
| --- | --- |
| simple pre-issued bearer/JWT tokens | token verifier / bearer auth |
| standards-compliant remote authorization server | RemoteAuthProvider-style integration |
| traditional upstream OAuth provider without MCP-native client registration behavior | OAuth proxy pattern |
| OIDC identity provider | documented OIDC proxy/provider pattern |
| multiple acceptable auth mechanisms | `MultiAuth` |
| client credentials / service-to-service client | appropriate client auth helper on FastMCP Client |
| STDIO local server | OS/process/launcher security; FastMCP HTTP auth is not the boundary |

Always verify issuer/audience/scope and map authenticated identity to object/tenant authorization. Never use a token merely because signature validation succeeds.

---

## Y) Client construction matrix

| Source passed to `Client(...)` | Meaning |
| --- | --- |
| `FastMCP` instance | in-memory transport; ideal for tests |
| `https://.../mcp` URL | Streamable HTTP inference |
| local Python/server path | STDIO/local server inference |
| explicit transport object | caller owns exact transport options |
| MCP config object/dict | configuration-driven transport |

Client lifecycle:

```python
async with Client(source) as client:
    await client.ping()
    tools = await client.list_tools()
    result = await client.call_tool("name", {...})
```

Use `client.new()` for a fresh independent session with the same configuration; reuse one active client when shared session state is intentional.

---

## Z) Client auth matrix

| Need | Client-side approach |
| --- | --- |
| fixed bearer token | token string / BearerAuth helper |
| interactive OAuth | `auth="oauth"` / OAuth helper |
| client credentials | documented client-credentials helper |
| CIMD-aware OAuth | CIMD support in current stable client/auth surface |
| no auth | explicit none/default as appropriate |
| STDIO | auth usually belongs to launched server/environment, not HTTP Authorization header |

Never reuse one response cache/client session across changing principals without explicit principal partitioning.

---

## AA) Apps / provider quick matrix

| App need | Primitive | Enforcement note |
| --- | --- | --- |
| one custom visual result | `@mcp.tool(app=True)` | backend tool security still applies |
| multi-tool app | `FastMCPApp` | app-only visibility is not auth |
| approval card | `Approval` | advisory only |
| choice buttons | `Choice` | advisory only |
| typed form | `FormInput` | Pydantic validation; business validation still needed |
| file upload | `FileUpload` | stable authenticated scope required in stateless HTTP |
| model-generated Prefab | `GenerativeUI` | sandbox + backend auth |
| custom web frontend | MCP Apps custom HTML/AppConfig | explicit CSP/permissions |

Production Apps: pin FastMCP **and** the tested `prefab-ui` version.

---

## AB) OpenAPI / FastAPI generation matrix

| Requirement | Mechanism |
| --- | --- |
| one generated API server | `FastMCP.from_openapi(...)` |
| generated source as part of larger server | `OpenAPIProvider(...)` |
| existing FastAPI app -> MCP | `FastMCP.from_fastapi(app=...)` |
| MCP server mounted into FastAPI | `mcp.http_app()` + `app.mount(...)` |
| route exclusion/type mapping | `RouteMap` + `MCPType` |
| huge generated catalog | curation + ToolTransform/Namespace + Tool Search |
| high-level business workflow | handwritten facade tool over generated/upstream operations |

Security: pin patched stable FastMCP; constrain upstream origins; test path traversal/SSRF/redirects; do not forward arbitrary inbound headers.

---

## AC) CLI workflow matrix

| Goal | Command |
| --- | --- |
| verify installed version | `fastmcp version` |
| launch local server | `fastmcp run ...` |
| protocol dev / Inspector | `fastmcp dev ...` |
| Apps preview | `fastmcp dev apps ...` |
| local manifest | `fastmcp inspect ... --format mcp` |
| live component list | `fastmcp list ...` |
| live tool/resource/prompt smoke call | `fastmcp call ...` |
| find configured local MCP servers | `fastmcp discover` |
| generate typed operator/agent CLI | `fastmcp generate-cli ...` |
| register server with supported host | `fastmcp install ...` |

`inspect` is local-definition-oriented; `list`/`call` exercise the live client/protocol path.

---

## AD) Contract-testing matrix

```text
Pure function unit test
  + Client(mcp) in-memory integration
  + protocol manifest snapshot
  + tool fingerprint manifest
  + auth/visibility matrix
  + actual STDIO/HTTP integration
  + target host/App acceptance
```

Fingerprint recipe inputs:

```text
tool.key
+ tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)
+ your selected stable fields
-> canonical sorted JSON
-> SHA-256
```

Treat description changes as behaviorally meaningful when model routing depends on them.

---

## AE) Security checklist — condensed

```text
[ ] Exact patched FastMCP version pinned.
[ ] Remote HTTP requires intentional authentication policy.
[ ] Authorization enforced on direct component access.
[ ] Object/tenant ownership enforced in business logic.
[ ] Visibility/Search never treated as authorization.
[ ] Host/Origin/proxy trust configured and tested.
[ ] OAuth issuer/audience/discovery/public URLs correct.
[ ] Inbound caller credential is not blindly forwarded upstream.
[ ] OpenAPI/proxy outbound origins constrained; SSRF tests present.
[ ] File access/upload rooted, canonicalized, size-limited, tenant-isolated.
[ ] Code Mode/Generative UI execute in sandbox with backend auth retained.
[ ] Approval UI not treated as hard security gate.
[ ] Cache keys/partitions include identity when response depends on identity.
[ ] Session/task stores have ACL, TTL, retention, encryption policy as needed.
[ ] Error details masked for public production endpoints.
[ ] Logs/traces redact secrets and preserve audit identifiers.
[ ] Rate, concurrency, result, file, and task quotas exist.
```

---

## AF) Performance/scaling checklist — condensed

```text
[ ] Shared expensive resources initialized in lifespan.
[ ] Async-native I/O uses async tool implementations.
[ ] Thread-affine sync tools use run_in_thread=False only when short/necessary.
[ ] Giant catalogs use Tool Search/Code Mode or deliberate curation.
[ ] Raw lists are paginated when useful.
[ ] Remote providers have timeout/failure isolation.
[ ] Clients reuse sessions when principal/session semantics permit.
[ ] Scaled HTTP does not rely on process-local cross-request state.
[ ] Durable tasks use shared backend/workers.
[ ] Identity-sensitive responses are not globally cached.
[ ] Tool schemas and results are bounded/facade-shaped.
[ ] Streaming behavior tested through real reverse proxy.
[ ] OTEL + metrics separate framework/provider/business latency.
```

---

## AG) V2 -> V3 migration grep sheet

```text
host= / port= in FastMCP(...)
log_level= / debug= / stateless_http= in FastMCP(...)
include_tags / exclude_tags
tool_serializer
on_duplicate_tools / resources / prompts
get_tools / get_resources / get_prompts
PromptMessage
component.enable() / component.disable()
import_server
as_proxy
server.proxy legacy imports
add_tool_transformation
_fastmcp metadata namespace
FASTMCP_SHOW_CLI_BANNER
FASTMCP_DECORATOR_MODE assumptions
ctx.get_state / set_state without await
```

Use the official upgrade guide for each hit rather than automated blind replacement.

---

## AH) Stable v3 vs FastMCP 4 beta boundary

| Topic | Stable 3.4.7 | V4 beta direction |
| --- | --- | --- |
| Primary reference in this document | Sections 0–35 | Section 36 only |
| Server callback sampling | `ctx.sample` / `sample_step` available when client supports it | removed on modern sessionless server context |
| Roots callback | available in v3 client/server callback model | removed from modern server callback path |
| Elicitation | `ctx.elicit(...)` mid-request | multi-round `InputRequiredResult` workflow |
| Cross-request application state | v3 session state | explicit `UserSession` / `SessionId` model |
| Background tasks | v3 task framework/extra | protocol extension + `fastmcp-tasks` |
| Protocol model Python fields | older camelCase-compatible surface | snake_case attributes; camelCase wire aliases |
| FastMCP HTTP stack | stable 3.x dependencies | moves to `httpx2` |
| Server extension system | limited/core v3 mechanisms | first-class negotiated extensions |
| Argument completions | not central stable-v3 surface | v4 server completion handlers |

**Agent invariant:** do not generate v4 beta code for a project pinned to 3.4.7, and do not assume the beta surface is frozen. Re-read current v4 docs before any production migration.

---

## AI) Production architecture quick chooser

| Context | Suggested pattern |
| --- | --- |
| single developer local repository tool | STDIO local server |
| shared internal service | authenticated Streamable HTTP + ASGI |
| multi-tenant enterprise | OAuth/OIDC + identity-aware DI + persistent stores |
| combine several MCPs | gateway with providers/namespaces/auth/search |
| existing REST API | OpenAPIProvider as lower layer + curated facade |
| 100s–1000s of tools | Tool Search; consider Code Mode for orchestration |
| multi-minute jobs | task frontend + shared backend + workers |
| interactive dashboard/form/upload | Apps/FastMCPApp + persistent scope where needed |
| REST + MCP same service | FastAPI + mounted `mcp.http_app()` with combined lifespan |
| stateless serverless | only if workflow does not depend on v3 session callbacks/process memory |

---

## AJ) Source-of-truth hierarchy for LLM coding agents

```text
1. Project's exact pinned FastMCP version / lockfile
2. Matching installed package signatures (`inspect.signature`, type checker, SDK docs for that version)
3. Official FastMCP SDK reference
4. Official FastMCP feature/deployment prose
5. Official release/changelog/upgrade guide
6. This reference document
7. Third-party blog examples only as inspiration, never signature authority
```

Current `gofastmcp.com` content can describe main-branch or prerelease behavior. Always reconcile a code example against the project's actual pinned major/minor before applying it.

### Additional appendix sources

* https://gofastmcp.com/getting-started/installation
* https://gofastmcp.com/changelog
* https://gofastmcp.com/updates
* https://gofastmcp.com/getting-started/upgrading/from-fastmcp-2
* https://gofastmcp.com/getting-started/upgrading/from-fastmcp-3
* https://gofastmcp.com/deployment/http
* https://gofastmcp.com/servers/tool-fingerprinting
* https://gofastmcp.com/cli/overview
* https://gofastmcp.com/apps/overview
* https://github.com/PrefectHQ/fastmcp/releases

