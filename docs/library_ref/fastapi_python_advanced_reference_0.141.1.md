# FastAPI in Python — advanced technical reference / feature-category catalog

## Version / source anchors

This reference is pinned to **FastAPI 0.141.1**, released **2026-07-29**, which is the latest stable FastAPI release on PyPI as of 2026-08-19. The package requires **Python >=3.10** and declares the `standard`, `standard-no-fastapi-cloud-cli`, and `all` extras. FastAPI is built on **Starlette** for the ASGI/web layer and **Pydantic** for validation, serialization, and schema generation.

For deployable code, treat the installed 0.141.1 package and version-matched API reference/release notes as the source of truth. FastAPI's documentation site tracks active development and can describe behavior added after a release; where an implementation detail is version-sensitive, pin the package, consult the release notes, and verify with a focused test.

Primary anchors used throughout:

- FastAPI docs: https://fastapi.tiangolo.com/
- FastAPI API reference: https://fastapi.tiangolo.com/reference/
- FastAPI release notes: https://fastapi.tiangolo.com/release-notes/
- PyPI release metadata: https://pypi.org/project/fastapi/
- Starlette docs: https://www.starlette.io/
- Pydantic docs: https://docs.pydantic.dev/
- Uvicorn docs: https://www.uvicorn.org/
- HTTPX docs: https://www.python-httpx.org/
- AnyIO docs: https://anyio.readthedocs.io/

## Why this reference is intentionally broader than a tutorial

FastAPI is often introduced as `FastAPI() + @app.get(...)`, but a production FastAPI application is an interaction among at least six layers:

1. **Python declaration layer** — type hints, `Annotated`, Pydantic models, dependency callables, return annotations.
2. **FastAPI contract layer** — parameter source inference, dependency graphs, validation, response models, OpenAPI generation.
3. **Starlette / ASGI layer** — routing, request/response objects, middleware, WebSockets, lifespan, static files, test client.
4. **Pydantic layer** — input parsing, constraints, output serialization, JSON Schema, settings where used.
5. **ASGI server layer** — Uvicorn or another ASGI server, event loop, workers, proxy headers, graceful shutdown.
6. **Deployment layer** — TLS termination, containers, orchestration, process supervision, observability, secrets, database lifecycle.

A serious agent reference therefore needs to document not only endpoint syntax, but also **ownership boundaries**: what FastAPI infers, what Starlette executes, what Pydantic validates, what Uvicorn serves, and what remains the application's responsibility.

## Feature inventory: what this reference covers

The public FastAPI capability surface naturally breaks into:

- application construction and global OpenAPI metadata;
- HTTP path operation registration and routing;
- automatic request-parameter source inference;
- path, query, header, cookie, form, file, and body parsing;
- Pydantic validation and JSON Schema;
- response models, response serialization, response classes, cookies and headers;
- ordinary streaming, JSON Lines streaming, and Server-Sent Events;
- dependency injection, sub-dependencies, caching, `yield` dependencies, and dependency scopes;
- lifespan, application state, and background tasks;
- HTTP auth schemes, OAuth2/OpenID Connect helpers, JWT-oriented patterns, and scopes;
- exception handling and validation-error customization;
- middleware, CORS, sessions, trusted hosts, gzip, and HTTPS redirects;
- `APIRouter`, router composition, route-tree behavior, and custom `APIRoute`/`APIRouter` classes;
- WebSockets;
- OpenAPI generation, Swagger UI/ReDoc, callbacks, webhooks, custom schemas, and generated SDKs;
- frontend/static-file serving, templates, sub-applications, mounts, and WSGI interop;
- settings and environment configuration;
- database/session lifetime patterns;
- synchronous and asynchronous testing;
- FastAPI CLI, Uvicorn, workers, proxies, HTTPS, containers, and orchestration;
- security hardening, performance, observability, large-application architecture, upgrades, and contract testing.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and FastAPI mental model
## 1) Installation, dependency policy, package extras, and project layout
## 2) First executable application, test, and CLI workflow
## 3) Architecture: ASGI, Starlette, Pydantic, and the request lifecycle
## 4) `FastAPI(...)` construction and application-wide configuration
## 5) Path operations, decorators, HTTP methods, and operation metadata
## 6) Parameter inference and request-source classification
## 7) Path, query, header, and cookie parameters
## 8) Request bodies, Pydantic models, nesting, examples, and schema behavior
## 9) Forms, multipart requests, files, and `UploadFile`
## 10) Response models, return annotations, filtering, and serialization
## 11) Response classes, status codes, cookies, headers, files, and redirects
## 12) Streaming responses and JSON Lines
## 13) Server-Sent Events (SSE)
## 14) Dependency injection fundamentals
## 15) Advanced dependencies: sub-dependencies, caching, `yield`, scope, and overrides
## 16) Lifespan, application state, and background tasks
## 17) Security primitives and OpenAPI security schemes
## 18) OAuth2, JWT, scopes, authentication, and authorization architecture
## 19) Errors, exceptions, request validation, and exception handlers
## 20) Middleware, CORS, TrustedHost, GZip, HTTPS redirect, and sessions
## 21) `APIRouter`, `include_router`, bigger applications, and the 0.137 route-tree architecture
## 22) Custom routing: `APIRoute`, `APIRouter`, request inspection, and advanced route control
## 23) WebSockets
## 24) OpenAPI generation, metadata, Swagger UI, ReDoc, and documentation configuration
## 25) Advanced OpenAPI: additional responses, callbacks, webhooks, custom schemas, and SDK generation
## 26) Frontend builds, static files, templates, mounts, sub-applications, and WSGI
## 27) Settings, environment variables, and configuration ownership
## 28) Database integration, transaction boundaries, and session dependencies
## 29) Testing with `TestClient`, lifespan, dependency overrides, and WebSockets
## 30) Async testing with HTTPX / ASGI transport
## 31) FastAPI CLI: `dev`, `run`, entrypoints, and `FASTAPI_ENV`
## 32) Uvicorn and ASGI server deployment
## 33) Worker processes, concurrency, async/threadpool boundaries, and process topology
## 34) Reverse proxies, HTTPS, proxy headers, `root_path`, and forwarded request metadata
## 35) Containers, orchestration, health, shutdown, and deployment topology
## 36) Security hardening and governance
## 37) Performance, dependency-graph scaling, validation, streaming, and large-app tuning
## 38) Large-application architecture and package boundaries
## 39) Observability, logging, tracing, metrics, and request correlation
## 40) API stability, Python/Pydantic/Starlette compatibility, and upgrade discipline
## 41) Current release delta: FastAPI 0.134 → 0.141.1
## 42) Production architecture patterns
## 43) Dense appendices and lookup matrices

---

# Stable release delta — why 0.141.1 deserves a new reference

The 2026 release line materially expanded FastAPI beyond the older “JSON REST endpoint” mental model:

| Release area | Material change | Why agents should care |
|---|---|---|
| 0.134.0 | first-class JSON Lines streaming | typed generator return annotations can define streamed item contracts |
| 0.135.0+ | first-class SSE support | FastAPI now has dedicated SSE response/event utilities rather than requiring a third-party pattern |
| 0.136.x | Python 3.14/free-threaded compatibility work and dependency upgrades | interpreter/runtime assumptions changed |
| 0.137.0 | major router composition/tree refactor | direct inspection/mutation of `router.routes` is no longer a safe application contract |
| 0.137.2 | `iter_route_contexts()` for advanced router inspection | sanctioned replacement for some old `router.routes` introspection |
| 0.138.0 | `app.frontend()` / `router.frontend()` | static SPA/site output became a first-class low-priority routing layer |
| 0.139.x | frontend dependency/fallback refinements and route-building fixes | frontend routes participate in FastAPI dependency/middleware behavior |
| 0.140.x | dependency-graph memory and OpenAPI performance refactors | large apps with deep/repeated dependency trees can behave materially better |
| 0.141.0 | `frontend(check_dir="auto")` | dev/prod frontend directory handling is environment-aware |
| 0.141.1 | frontend background-task/header dependency fix; `FASTAPI_ENV` docs | latest stable correctness boundary |

Source: FastAPI release notes, https://fastapi.tiangolo.com/release-notes/

---

# Source index used throughout this reference

The source labels used in section source lists refer to these primary documentation families:

- **[FA-HOME]** FastAPI home / install / package dependencies — https://fastapi.tiangolo.com/
- **[FA-REF]** FastAPI code/API reference — https://fastapi.tiangolo.com/reference/
- **[FA-REL]** FastAPI release notes — https://fastapi.tiangolo.com/release-notes/
- **[FA-TUT]** Tutorial/user guide — https://fastapi.tiangolo.com/tutorial/
- **[FA-ADV]** Advanced user guide — https://fastapi.tiangolo.com/advanced/
- **[FA-DEPLOY]** Deployment guide — https://fastapi.tiangolo.com/deployment/
- **[FA-CLI]** FastAPI CLI guide — https://fastapi.tiangolo.com/fastapi-cli/
- **[STARLETTE]** Starlette documentation — https://www.starlette.io/
- **[PYDANTIC]** Pydantic documentation — https://docs.pydantic.dev/
- **[UVICORN]** Uvicorn documentation — https://www.uvicorn.org/
- **[HTTPX]** HTTPX documentation — https://www.python-httpx.org/
- **[ANYIO]** AnyIO documentation — https://anyio.readthedocs.io/

---

# FastAPI Advanced — 0) Scope, versioning, and mental model

## 0.0 Version anchors and documentation stance

This document targets **FastAPI 0.141.1** and **Python 3.10+**. Use an exact or tightly constrained FastAPI pin in production when generated OpenAPI, dependency behavior, router internals, or serialization details are part of a tested contract.

Recommended project pin:

```toml
[project]
requires-python = ">=3.10"
dependencies = [
    "fastapi[standard]==0.141.1",
]
```

Or with uv:

```bash
uv add "fastapi[standard]==0.141.1"
```

For an intentionally smaller server-only dependency surface:

```bash
uv add "fastapi==0.141.1" uvicorn
```

**Agent rule:** never infer an exact FastAPI call signature from an old tutorial snippet when the API reference for the pinned version is available. Constructor, router, dependency, and streaming semantics have changed across recent releases.

## 0.1 Identity: what FastAPI is and is not

FastAPI is an **ASGI web framework focused on type-driven API construction**. It combines Starlette's ASGI/routing/request/response infrastructure with Pydantic's parsing/schema model and adds a layer that turns Python signatures into:

- request extraction rules;
- validation rules;
- dependency graphs;
- response serialization contracts;
- OpenAPI operations and JSON Schema components;
- interactive API documentation.

FastAPI is **not**:

- an ASGI server — Uvicorn/Hypercorn/etc. serve it;
- an ORM or database layer;
- an authorization policy engine by itself;
- a task queue;
- a distributed process supervisor;
- a replacement for reverse-proxy/TLS infrastructure;
- a guarantee that CPU-bound Python work is parallelized.

## 0.2 The shortest correct system model

```text
HTTP / WebSocket client
        │
        ▼
ASGI server (e.g. Uvicorn)
        │ scope / receive / send
        ▼
Starlette/FastAPI middleware stack
        │
        ▼
router match
        │
        ▼
FastAPI dependency + parameter graph
        ├─ path/query/header/cookie/form/file/body extraction
        ├─ dependency resolution and caching
        └─ Pydantic validation/coercion
        │
        ▼
path operation callable
        │
        ▼
response handling
        ├─ Response returned directly → pass through
        ├─ generator/stream → streaming path
        └─ Python value → response-model filtering/serialization
        │
        ▼
Starlette Response / ASGI send
        │
        ▼
client
```

OpenAPI is produced from much of the same declaration graph, but it is a **contract representation**, not the runtime router itself.

## 0.3 Declaration-time vs request-time work

A high-quality FastAPI design distinguishes work done when the application imports/builds from work done per request.

| Time | Typical work | Examples |
|---|---|---|
| import / app construction | route registration, dependency graph construction, OpenAPI metadata setup | `app = FastAPI()`, decorators, `include_router()` |
| startup / lifespan | initialize shared clients/pools/models | DB engine, HTTP client, model weights |
| request matching | choose route or frontend/static fallback | method/path matching |
| request dependency phase | extract/validate inputs and resolve dependencies | auth, DB session, headers |
| handler | domain operation | query/update/orchestrate |
| response phase | validate/filter/serialize or stream | response model, SSE, JSONL |
| dependency cleanup | `yield` teardown according to scope | transaction/session close |
| shutdown / lifespan | close shared resources | pools, clients, telemetry exporters |

Putting expensive shared initialization in module import code makes tests/imports slower and bypasses lifecycle semantics. Putting a connection pool constructor inside a per-request dependency recreates expensive infrastructure. Use lifespan for application-owned shared resources and dependencies for request-scoped resources.

## 0.4 Core object ownership map

| Object | Primary ownership role |
|---|---|
| `FastAPI` | application, root router, OpenAPI metadata, dependency overrides, middleware/lifespan integration |
| `APIRouter` | reusable route subtree with prefixes/tags/dependencies/responses/lifespan |
| `APIRoute` | compiled HTTP route contract and dependency/response metadata |
| `Depends` / `Security` | dependency declaration metadata |
| `Request` | direct Starlette request access for current HTTP request |
| `Response` | direct outgoing response mutation or full response control |
| Pydantic `BaseModel` | typed parsed/validated data and JSON Schema source |
| `BackgroundTasks` | in-process tasks executed after response handling |
| `WebSocket` | Starlette WebSocket connection interface |
| `TestClient` | Starlette/HTTPX-backed synchronous ASGI test client |

## 0.5 FastAPI vs Starlette boundary

FastAPI subclasses Starlette and intentionally reuses Starlette components. High-value inherited/re-exported features include:

- `Request`, `Response`, `WebSocket`;
- middleware infrastructure;
- `StaticFiles`, templates, mounts;
- `TestClient`;
- background tasks;
- ASGI lifespan semantics.

The FastAPI-specific layer adds type/schema/dependency/OpenAPI behavior. If a feature is fundamentally about raw HTTP or ASGI mechanics rather than type-driven API declaration, the definitive behavior may live in Starlette.

## 0.6 FastAPI vs Pydantic boundary

Pydantic handles the data-model machinery used by FastAPI, including:

- field constraints and parsing;
- `BaseModel` validation;
- JSON Schema generation;
- serialization behavior;
- `Annotated` metadata integration.

FastAPI decides **where input comes from** and **when to invoke Pydantic**. Pydantic does not know that `item_id` is a path parameter or that an `Authorization` header came from an OAuth2 scheme; FastAPI supplies that context.

## 0.7 OpenAPI contract generation

FastAPI uses route declarations, dependencies, parameter helpers, security schemes, response models, and metadata to generate OpenAPI. The default endpoints are:

```text
/openapi.json
/docs
/redoc
```

These can be moved or disabled. OpenAPI serves at least four roles:

1. interactive documentation;
2. client/SDK generation;
3. contract testing and drift review;
4. gateway/API-management ingestion.

Treat it as a production artifact when external consumers depend on it.

## 0.8 Agent invariants

1. **A type hint is not just editor metadata.** In FastAPI it can drive request parsing, validation, response serialization, and OpenAPI.
2. **Parameter location is inferred unless explicitly overridden.** Path name, scalar type/default, Pydantic model, and helper annotations influence source selection.
3. **Dependencies form a graph, not a flat callback list.** Sub-dependencies are deduplicated/cached by default per request.
4. **Returning `Response` bypasses normal response-model serialization.** Do it intentionally.
5. **`async def` is not automatically faster.** It is correct when the implementation awaits non-blocking I/O; blocking calls in an async handler block the event loop.
6. **OpenAPI is generated contract metadata, not runtime access control.** Hiding a route from schema does not secure it.
7. **Middleware and dependency order are semantic.** Security, state, headers, errors, and tracing can change with placement.
8. **Version pinning matters.** The 0.137 router changes and 0.140 dependency refactors are concrete examples.

## 0.9 Anti-pattern inventory

- Calling FastAPI an HTTP server and omitting Uvicorn/ASGI runtime from architecture diagrams.
- Treating Pydantic models as ORM entities or transaction managers.
- Putting all business logic directly inside route functions.
- Using `Request` everywhere and bypassing typed parameter/dependency declarations without need.
- Hiding a route from OpenAPI and assuming it is private.
- Creating per-request resources that should be application-singletons.
- Creating global resources at import time when they require startup/shutdown cleanup.
- Returning raw `Response` subclasses by default and losing schema/serialization benefits.
- Inspecting private router internals as a stable API.

## 0.10 Agent checklist

```text
[ ] Confirm pinned FastAPI version.
[ ] Confirm supported Python version.
[ ] Identify Starlette-owned vs FastAPI-owned behavior.
[ ] Identify Pydantic validation/serialization boundary.
[ ] Separate import, lifespan, request, response, and shutdown lifetimes.
[ ] Treat OpenAPI as a contract artifact where relevant.
[ ] Keep auth enforcement separate from documentation visibility.
[ ] Decide async vs sync based on I/O behavior, not fashion.
[ ] Avoid private router/dependency internals unless section 22/40 explicitly justifies them.
```

### Sources

- [FA-HOME] https://fastapi.tiangolo.com/
- [FA-REF] https://fastapi.tiangolo.com/reference/fastapi/
- [FA-REL] https://fastapi.tiangolo.com/release-notes/
- [STARLETTE] https://www.starlette.io/
- [PYDANTIC] https://docs.pydantic.dev/

---

# FastAPI Advanced — 1) Installation, dependency policy, package extras, and project layout

## 1.0 Release target

```bash
uv add "fastapi[standard]==0.141.1"
```

The `standard` extra is the normal batteries-included install recommended by current FastAPI docs. It includes dependencies used for common production/dev workflows such as:

- `uvicorn[standard]` for serving;
- `fastapi-cli[standard]` for the `fastapi` command;
- `httpx` for `TestClient`;
- `jinja2` for default templating support;
- `python-multipart` for form parsing;
- `email-validator` for Pydantic email validation.

If you do not want those optional dependencies:

```bash
uv add "fastapi==0.141.1"
```

If you want standard dependencies but not FastAPI Cloud CLI:

```bash
uv add "fastapi[standard-no-fastapi-cloud-cli]==0.141.1"
```

## 1.1 Python version policy

FastAPI 0.141.1 requires Python >=3.10. A production project should normally choose a narrower interpreter support window, for example:

```toml
[project]
requires-python = ">=3.12,<3.15"
```

Why narrow it?

- deployment image consistency;
- reproducible dependency resolver behavior;
- predictable event-loop/runtime performance;
- fewer branches in application typing and compatibility tests.

FastAPI supports Python 3.10–3.14 in current package metadata. Test your own dependency stack before assuming every database driver, binary extension, or observability package has equal support.

## 1.2 Optional application dependencies

FastAPI intentionally does not bundle database/ORM/business-stack choices. Common explicit additions:

```bash
uv add pydantic-settings
uv add sqlalchemy asyncpg
uv add alembic
uv add pyjwt "pwdlib[argon2]"
uv add structlog
uv add opentelemetry-api opentelemetry-sdk
```

These are examples, not FastAPI requirements. Keep framework reference code from implying that FastAPI mandates a particular ORM, JWT package, logger, or telemetry backend.

## 1.3 `pyproject.toml` application pattern

```toml
[project]
name = "my-api"
version = "0.1.0"
requires-python = ">=3.12,<3.15"
dependencies = [
  "fastapi[standard]==0.141.1",
  "pydantic-settings>=2,<3",
]

[dependency-groups]
dev = [
  "pytest>=8",
  "pytest-cov>=6",
  "ruff>=0.12",
  "mypy>=1.17",
]

[tool.fastapi]
entrypoint = "app.main:app"
```

The `[tool.fastapi] entrypoint` lets the CLI locate the application without repeating a file/module argument.

## 1.4 Suggested production package layout

```text
my-api/
├── pyproject.toml
├── uv.lock
├── README.md
├── app/
│   ├── __init__.py
│   ├── main.py                 # composition root; FastAPI() + routers + middleware
│   ├── config.py               # Settings model / environment parsing
│   ├── lifespan.py             # startup/shutdown-owned resources
│   ├── dependencies.py         # cross-cutting request dependencies
│   ├── api/
│   │   ├── __init__.py
│   │   ├── router.py           # top-level APIRouter
│   │   └── routes/
│   │       ├── users.py
│   │       └── orders.py
│   ├── schemas/                # Pydantic input/output contract models
│   ├── services/               # use-case/application logic
│   ├── domain/                 # optional domain objects/rules
│   ├── repositories/           # persistence adapters
│   ├── infrastructure/         # DB/HTTP/cache/etc. implementations
│   └── observability.py
└── tests/
    ├── conftest.py
    ├── api/
    ├── integration/
    └── contract/
```

The goal is not the exact folders; the goal is **keeping transport declaration separate from business behavior and infrastructure lifecycle**.

## 1.5 `main.py` as composition root

```python
from fastapi import FastAPI

from app.api.router import api_router
from app.lifespan import lifespan


def create_app() -> FastAPI:
    app = FastAPI(
        title="Example API",
        version="1.0.0",
        lifespan=lifespan,
        strict_content_type=True,
    )
    app.include_router(api_router, prefix="/api")
    return app


app = create_app()
```

Factory-style construction is useful when tests need differently configured apps, but do not create a new app on every request.

## 1.6 Lockfiles and pinning

For an application rather than a reusable library, commit a lockfile. With uv:

```bash
uv lock
uv sync --frozen
```

A practical version policy:

```text
FastAPI / Starlette / Pydantic runtime: lock exact resolved versions.
Application direct deps: constrain intentionally.
Production builds: install from lockfile.
Upgrade PR: update lock + run OpenAPI/behavior tests.
```

## 1.7 Minimal vs standard install decision matrix

| Need | Install stance |
|---|---|
| normal API dev + CLI + tests | `fastapi[standard]` |
| custom ASGI server and no CLI/test extras | `fastapi` + chosen server |
| standard without cloud CLI | `fastapi[standard-no-fastapi-cloud-cli]` |
| forms/files | ensure `python-multipart` is installed |
| `TestClient` | ensure `httpx` is installed |
| Jinja2 templates | ensure `jinja2` is installed |
| `EmailStr` | ensure `email-validator` is installed |
| `ORJSONResponse` | install `orjson` |
| settings models | install `pydantic-settings` |

## 1.8 Developer commands

```bash
# dependency install
uv sync

# local autoreload server
uv run fastapi dev

# production-shaped server
uv run fastapi run

# tests
uv run pytest

# lint/format examples
uv run ruff check .
uv run ruff format --check .
```

## 1.9 Import-time health check

```bash
uv run python - <<'PY'
import fastapi
from app.main import app
print("fastapi", fastapi.__version__)
print("app", app.title)
PY
```

Do this in CI before container build/publish so obvious import and dependency wiring failures fail early.

## 1.10 Project dependency boundaries

Recommended ownership:

| Concern | Own it in |
|---|---|
| ASGI API framework | FastAPI |
| HTTP primitives | Starlette/FastAPI |
| schema/validation | Pydantic |
| settings | `pydantic-settings` or explicit alternative |
| DB engine/session | database library / infrastructure module |
| migrations | migration tool |
| business logic | application/domain layer |
| auth tokens/identity provider | security/infrastructure layer |
| worker queue | external task system when durability/distribution required |
| metrics/traces | observability stack |

## 1.11 Anti-pattern inventory

- `pip install fastapi` and later assuming `fastapi` CLI, form parsing, or `TestClient` dependencies are necessarily installed.
- Depending on an unbounded FastAPI version for a production app.
- Re-exporting every framework symbol through a giant project `utils.py`.
- Building route, database, config, and domain logic in one `main.py`.
- Importing production secrets at module import as constants when test/config overrides are required.
- Treating package extras as a security boundary; extras are convenience dependency groups.

## 1.12 Agent checklist

```text
[ ] Pin FastAPI 0.141.1 for this reference.
[ ] Use Python >=3.10; choose a narrower production range if practical.
[ ] Prefer fastapi[standard] unless deliberately minimizing.
[ ] Add pydantic-settings explicitly if using Settings models.
[ ] Commit a lockfile for applications.
[ ] Put [tool.fastapi] entrypoint in pyproject.toml when useful.
[ ] Keep app composition separate from routes/services/infrastructure.
[ ] Verify import + FastAPI version in CI.
```

### Sources

- [FA-HOME] https://fastapi.tiangolo.com/#installation
- PyPI: https://pypi.org/project/fastapi/
- CLI entrypoint: https://fastapi.tiangolo.com/tutorial/first-steps/

---

# FastAPI Advanced — 2) First executable application, test, and CLI workflow

## 2.0 Objective

The first executable should prove the complete minimum path:

```text
Python declaration
→ FastAPI route registration
→ ASGI server
→ request parsing
→ Pydantic/FastAPI validation
→ handler execution
→ response serialization
→ OpenAPI/docs generation
→ in-process test
```

## 2.1 Minimal application

`app/main.py`:

```python
from fastapi import FastAPI

app = FastAPI(title="First API", version="0.1.0")


@app.get("/")
async def root() -> dict[str, str]:
    return {"message": "hello"}


@app.get("/items/{item_id}")
async def read_item(item_id: int, q: str | None = None) -> dict[str, object]:
    return {"item_id": item_id, "q": q}
```

FastAPI infers:

- `item_id` is a path parameter because `{item_id}` occurs in the route path;
- its `int` annotation drives parsing/validation;
- `q` is a query parameter because it is not in the path and is scalar;
- `q` is optional because its default is `None`;
- return values are JSON-encoded into the default response class.

## 2.2 Run with the FastAPI CLI

If `[tool.fastapi] entrypoint = "app.main:app"` is configured:

```bash
uv run fastapi dev
```

For a production-shaped process:

```bash
uv run fastapi run
```

Or explicit source:

```bash
uv run fastapi dev app/main.py
```

## 2.3 Verify protocol surfaces

```bash
curl -i http://127.0.0.1:8000/
curl -i http://127.0.0.1:8000/items/42?q=test
curl -i http://127.0.0.1:8000/items/not-an-int
```

Then inspect:

```text
http://127.0.0.1:8000/docs
http://127.0.0.1:8000/redoc
http://127.0.0.1:8000/openapi.json
```

The validation failure is an important first smoke test: it proves that the route signature is participating in the request contract rather than being an untyped wrapper.

## 2.4 First Pydantic request/response model

```python
from pydantic import BaseModel, Field


class ItemCreate(BaseModel):
    name: str = Field(min_length=1, max_length=200)
    price: float = Field(gt=0)


class ItemRead(BaseModel):
    id: int
    name: str
    price: float


@app.post("/items", response_model=ItemRead, status_code=201)
async def create_item(payload: ItemCreate) -> ItemRead:
    return ItemRead(id=1, **payload.model_dump())
```

This proves both directions:

```text
request JSON -> ItemCreate validation
Python return -> ItemRead response contract -> JSON
```

## 2.5 First dependency

```python
from typing import Annotated
from fastapi import Depends, Header, HTTPException


async def require_api_key(x_api_key: Annotated[str | None, Header()] = None) -> str:
    if x_api_key != "dev-secret":
        raise HTTPException(status_code=401, detail="Invalid API key")
    return x_api_key


ApiKey = Annotated[str, Depends(require_api_key)]


@app.get("/private")
async def private_endpoint(api_key: ApiKey) -> dict[str, bool]:
    return {"ok": True}
```

The dependency does not merely call a helper; it becomes part of the route's runtime dependency graph and can contribute OpenAPI parameter/security information depending on the declaration used.

## 2.6 First lifespan resource

```python
from contextlib import asynccontextmanager
from fastapi import FastAPI


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.catalog = {"1": {"name": "example"}}
    try:
        yield
    finally:
        app.state.catalog.clear()


app = FastAPI(lifespan=lifespan)
```

Use lifespan for application-owned resources that need deterministic startup/shutdown.

## 2.7 First synchronous test

```python
from fastapi.testclient import TestClient
from app.main import app


def test_root() -> None:
    with TestClient(app) as client:
        response = client.get("/")
    assert response.status_code == 200
    assert response.json() == {"message": "hello"}
```

Using `with TestClient(app)` activates lifespan. A bare `client = TestClient(app)` is fine for apps without lifespan requirements, but it does not give the same explicit lifecycle boundary in the test.

## 2.8 First OpenAPI contract test

```python
from fastapi.testclient import TestClient
from app.main import app


def test_openapi_contract_has_items() -> None:
    with TestClient(app) as client:
        schema = client.get("/openapi.json").json()

    assert schema["info"]["title"] == "First API"
    assert "/items/{item_id}" in schema["paths"]
```

For a public API, persist a normalized OpenAPI artifact in CI and review intentional changes.

## 2.9 First dependency override test

```python
from app.main import app, require_api_key


def override_api_key() -> str:
    return "test-key"


def test_private(client):
    app.dependency_overrides[require_api_key] = override_api_key
    try:
        response = client.get("/private")
        assert response.status_code == 200
    finally:
        app.dependency_overrides.clear()
```

Always clean overrides to prevent test-order coupling.

## 2.10 First production-shaped file split

```text
app/main.py
    -> FastAPI(), middleware, lifespan, include_router
app/api/router.py
    -> aggregate feature routers
app/api/routes/items.py
    -> HTTP declaration only
app/services/items.py
    -> use-case logic
app/schemas/items.py
    -> Pydantic contract models
app/infrastructure/db.py
    -> DB engine/session integration
```

## 2.11 Sync vs async first rule

Use:

```python
@app.get("/remote")
async def remote():
    result = await async_http_client.get(...)
    return result.json()
```

for non-blocking awaitable libraries.

Use:

```python
@app.get("/legacy")
def legacy():
    return blocking_library_call()
```

when the underlying library is synchronous/blocking. FastAPI/Starlette can run normal `def` path operations in a threadpool. Do not call a long blocking function directly inside `async def` unless you explicitly offload it.

## 2.12 First-app failure modes

| Symptom | Likely issue |
|---|---|
| CLI cannot find app | wrong module/file or missing `[tool.fastapi] entrypoint` |
| form endpoint crashes at startup | missing `python-multipart` |
| `TestClient` missing dependency | no `httpx` in minimal install |
| lifespan state not initialized in test | `TestClient` not used as context manager |
| async endpoint stalls concurrent traffic | blocking I/O/CPU executed on event loop |
| validation does not match expectation | incorrect parameter source/default/type annotation |
| docs expose wrong schema | response/request annotation or OpenAPI metadata mismatch |

## 2.13 Agent checklist

```text
[ ] Run one route through CLI/server.
[ ] Verify path + query inference.
[ ] Verify one validation failure.
[ ] Verify docs and openapi.json.
[ ] Add one Pydantic request/response model.
[ ] Add one dependency.
[ ] Add lifespan only for real shared lifecycle resources.
[ ] Test through TestClient, not only direct Python calls.
[ ] Add OpenAPI contract smoke test.
[ ] Decide sync vs async based on underlying library behavior.
```

### Sources

- First steps: https://fastapi.tiangolo.com/tutorial/first-steps/
- Testing: https://fastapi.tiangolo.com/tutorial/testing/
- Lifespan testing: https://fastapi.tiangolo.com/advanced/testing-events/
- Dependencies: https://fastapi.tiangolo.com/tutorial/dependencies/

---
# FastAPI Advanced — 3) Architecture: ASGI, Starlette, Pydantic, and the request lifecycle

## 3.0 Why architecture boundaries matter

FastAPI's ergonomics intentionally hide ASGI boilerplate. That is useful until an application needs custom middleware, streaming, WebSockets, direct `Request` access, proxy behavior, lifespan, or custom routing. At that point, an agent must know which layer owns the behavior.

```text
Uvicorn/ASGI server
   │
   │ ASGI scope + receive/send callables
   ▼
FastAPI (Starlette subclass)
   │
   ├─ middleware stack
   ├─ router tree
   ├─ exception handlers
   ├─ lifespan
   └─ state
        │
        ▼
FastAPI route dependency/validation layer
        │
        ├─ Pydantic model validation
        ├─ parameter helpers
        └─ dependency solving
        │
        ▼
user path operation
```

## 3.1 ASGI contract

An ASGI application is conceptually an async callable:

```python
async def app(scope, receive, send):
    ...
```

The server constructs a `scope` describing a connection/request and gives the app channels to receive incoming events and send outgoing events. HTTP and WebSocket scopes have different event protocols. FastAPI normally shields application code from this level; custom ASGI middleware or low-level streaming may interact with it directly.

**Agent rule:** do not write raw ASGI middleware when Starlette's `BaseHTTPMiddleware` or a normal FastAPI/Starlette middleware class is sufficient, but understand raw ASGI when streaming/context propagation/cancellation semantics make higher-level middleware inappropriate.

## 3.2 Starlette ownership

FastAPI directly subclasses `Starlette`. Core Starlette-owned concepts include:

- ASGI application lifecycle;
- route matching primitives;
- `Request` and `Response`;
- `WebSocket`;
- middleware;
- static files and templates;
- `TestClient`;
- background task primitives;
- lifespan mechanics.

FastAPI layers route signature analysis, Pydantic validation, dependencies, and OpenAPI on top.

## 3.3 Pydantic ownership

Pydantic does not decide whether a value came from a query parameter, header, or JSON body. FastAPI extracts raw values and asks Pydantic/type adapters to validate them against the declared type/constraints.

Example:

```python
from typing import Annotated
from fastapi import Query
from pydantic import BaseModel, Field

class Filter(BaseModel):
    limit: int = Field(default=20, ge=1, le=100)

@app.get("/search")
async def search(q: Annotated[str, Query(min_length=2)]) -> dict:
    return {"q": q}
```

FastAPI owns the fact that `q` comes from the query string. Pydantic validation machinery enforces the string constraints and produces schema metadata.

## 3.4 HTTP request lifecycle — expanded

```text
1. ASGI server accepts socket/request.
2. Middleware begins outer-to-inner traversal.
3. Router resolves method + path.
4. FastAPI identifies the APIRoute endpoint.
5. Dependency solver resolves request graph:
   a. path values
   b. query/header/cookie values
   c. body/form/file values
   d. dependency callables and sub-dependencies
   e. security dependencies
6. Input validation failures become RequestValidationError.
7. Path operation callable executes.
8. Return value is handled:
   a. Response instance -> direct
   b. streaming/generator -> streaming machinery
   c. other value -> response field/model validation + encoding
9. Response travels outward through middleware.
10. Background tasks run according to response lifecycle.
11. yield-dependency cleanup occurs according to dependency scope.
12. Server completes ASGI response.
```

The precise ordering of streaming, background tasks, and dependency cleanup depends on the dependency scope and response path. For resource lifetime-sensitive code, test it explicitly.

## 3.5 Path-operation function vs route object

The decorated Python function remains useful application code, but the runtime endpoint is represented by an `APIRoute` that carries compiled metadata about:

- path and methods;
- endpoint callable;
- dependency graph;
- response field/model;
- status code;
- route name;
- tags, responses, callbacks, OpenAPI inclusion;
- route-specific custom class behavior.

Do not assume replacing function attributes after route registration rebuilds the route contract.

## 3.6 Sync function execution

Normal `def` path operations and dependencies may run in a threadpool so they do not block the event loop directly. This is a compatibility boundary for synchronous I/O libraries, not permission to perform arbitrarily expensive CPU work.

```python
@app.get("/sync")
def sync_endpoint():
    return blocking_client.fetch()
```

Threadpool capacity is finite. Hundreds of long blocking calls can saturate it. CPU-bound work still contends for CPU/GIL behavior unless using processes/native code/free-threaded-safe execution.

## 3.7 Async function execution

```python
@app.get("/async")
async def async_endpoint():
    result = await async_client.fetch()
    return result
```

Within `async def`, every blocking call that takes meaningful time is a potential event-loop stall:

```python
# bad inside async endpoint
import time
time.sleep(5)
```

Prefer:

```python
await asyncio.sleep(5)
```

or use an async client/offloading mechanism for blocking libraries.

## 3.8 Request-local vs application-local state

| State | Best home |
|---|---|
| request headers/client/path | `Request`, typed parameters |
| authenticated principal | dependency return value / request state when middleware owns it |
| DB transaction/session | request-scoped dependency |
| shared DB pool/engine | lifespan-created app state / module-owned configured singleton with cleanup |
| HTTP client pool | lifespan |
| immutable configuration | settings object injected/shared |
| per-request trace ID | middleware/context variable/request state |

## 3.9 Exception propagation

FastAPI registers exception handlers for expected framework errors. Custom application exceptions can be mapped with `@app.exception_handler(...)` or `exception_handlers={...}`. Middleware may catch/transform exceptions depending on its position.

Do not blanket-catch `Exception` inside every route just to return JSON; doing so bypasses central logging/exception policy and often turns programming bugs into misleading 200/400 responses.

## 3.10 Architecture anti-patterns

- Treating ASGI server workers as threads inside one FastAPI object.
- Calling blocking database drivers in `async def` without offloading.
- Storing per-request mutable data globally.
- Creating application-wide network clients per request.
- Using middleware for domain authorization when dependency-based policy would be clearer.
- Using dependencies for every generic HTTP header mutation when middleware is a more appropriate global layer.
- Coupling service/domain code to `Request` or `Depends` unnecessarily.

## 3.11 Agent checklist

```text
[ ] Identify ASGI server, FastAPI, Starlette, and Pydantic boundaries.
[ ] Identify request-scoped vs application-scoped resources.
[ ] Keep blocking calls out of event loop.
[ ] Use route/dependency concepts rather than mutating function metadata post-registration.
[ ] Centralize exception mapping.
[ ] Test streaming/cleanup ordering when resource lifetime matters.
```

### Sources

- FastAPI class: https://fastapi.tiangolo.com/reference/fastapi/
- Async/concurrency: https://fastapi.tiangolo.com/async/
- Starlette: https://www.starlette.io/
- Pydantic: https://docs.pydantic.dev/

---

# FastAPI Advanced — 4) `FastAPI(...)` construction and application-wide configuration

## 4.0 Exact constructor anchor for 0.141.1

The current reference surface is:

```python
FastAPI(
    *,
    debug=False,
    routes=None,
    title="FastAPI",
    summary=None,
    description="",
    version="0.1.0",
    openapi_url="/openapi.json",
    openapi_tags=None,
    servers=None,
    dependencies=None,
    default_response_class=Default(JSONResponse),
    redirect_slashes=True,
    docs_url="/docs",
    redoc_url="/redoc",
    swagger_ui_oauth2_redirect_url="/docs/oauth2-redirect",
    swagger_ui_init_oauth=None,
    middleware=None,
    exception_handlers=None,
    on_startup=None,
    on_shutdown=None,
    lifespan=None,
    terms_of_service=None,
    contact=None,
    license_info=None,
    openapi_prefix="",
    root_path="",
    root_path_in_servers=True,
    responses=None,
    callbacks=None,
    webhooks=None,
    deprecated=None,
    include_in_schema=True,
    swagger_ui_parameters=None,
    generate_unique_id_function=Default(generate_unique_id),
    separate_input_output_schemas=True,
    openapi_external_docs=None,
    strict_content_type=True,
    **extra,
)
```

The constructor inherits Starlette compatibility fields while adding OpenAPI/FastAPI-specific configuration.

## 4.1 Production-oriented constructor example

```python
from fastapi import FastAPI
from fastapi.responses import ORJSONResponse

app = FastAPI(
    title="Orders API",
    summary="Order management service",
    description="Internal and partner-facing order operations.",
    version="2.3.0",
    docs_url="/docs",
    redoc_url=None,
    openapi_url="/openapi.json",
    default_response_class=ORJSONResponse,
    lifespan=lifespan,
    strict_content_type=True,
    generate_unique_id_function=operation_id,
)
```

Use `ORJSONResponse` only if `orjson` is installed and benchmarked/desired. The default `JSONResponse` is usually sufficient.

## 4.2 Identity and OpenAPI metadata fields

Key fields:

- `title`, `summary`, `description`, `version`;
- `terms_of_service`;
- `contact`;
- `license_info`;
- `openapi_tags`;
- `servers`;
- `openapi_external_docs`.

These affect the generated API contract and documentation, not runtime business semantics.

## 4.3 Docs/OpenAPI URL policy

```python
app = FastAPI(
    openapi_url="/schema/openapi.json",
    docs_url="/schema/docs",
    redoc_url=None,
)
```

Disable public docs only when product/security policy requires it:

```python
app = FastAPI(openapi_url=None, docs_url=None, redoc_url=None)
```

Note: if `openapi_url=None`, the automatic docs cannot function because they rely on the schema.

**Security invariant:** disabling docs does not make endpoints inaccessible.

## 4.4 Global dependencies

```python
app = FastAPI(
    dependencies=[Depends(require_request_context)],
)
```

Global dependencies run for every applicable path operation. Use them for truly global request policy such as tenant context validation or API-wide authentication. Avoid hiding unrelated side effects in a global dependency because every route inherits it.

## 4.5 Global default response class

```python
app = FastAPI(default_response_class=ORJSONResponse)
```

This changes the default response class for routes without an explicit override. It does not mean every returned `Response` is rewrapped; an explicit `Response` still passes through.

## 4.6 `debug`

`debug=True` can expose detailed tracebacks on server errors. Keep it off in production unless you have a tightly controlled environment. Use structured logging/error reporting instead.

## 4.7 `redirect_slashes`

With slash redirection enabled, `/items` and `/items/` mismatches can result in redirects depending on route definition. In APIs where redirects are undesirable for clients/signatures/caching, standardize paths and consider explicit policy.

## 4.8 `lifespan` over legacy event lists

Current FastAPI guidance prefers lifespan context management:

```python
@asynccontextmanager
async def lifespan(app: FastAPI):
    ...
    yield
    ...

app = FastAPI(lifespan=lifespan)
```

Legacy `on_startup` / `on_shutdown` compatibility remains visible on constructor/reference surfaces, but new code should prefer lifespan unless integrating legacy components.

## 4.9 `root_path` and proxy deployment

`root_path` represents an ASGI deployment prefix when the app is exposed behind a proxy under a subpath. It is not generally the same as putting a prefix on every route.

```python
app = FastAPI(root_path="/api/v1")
```

Use it only when deployment topology requires it. See section 34.

## 4.10 `separate_input_output_schemas`

When enabled (default), FastAPI can generate different JSON Schemas for the input and output uses of a model when Pydantic semantics differ. This is useful when defaults/requiredness/serialization result in distinct contracts.

Do not disable it merely to reduce schema component count without evaluating generated-client implications.

## 4.11 `strict_content_type=True`

This current default makes JSON-body parsing require a valid JSON Content-Type. It helps avoid a specific class of browser CSRF risk for local/internal unauthenticated JSON APIs and makes request contracts stricter.

If legacy clients omit Content-Type and you must support them:

```python
app = FastAPI(strict_content_type=False)
```

Treat that as a compatibility concession and reassess security assumptions.

## 4.12 `generate_unique_id_function`

OpenAPI operation IDs often feed SDK generation. A custom function can stabilize naming across modules:

```python
from fastapi.routing import APIRoute


def operation_id(route: APIRoute) -> str:
    method = sorted(route.methods or {"GET"})[0].lower()
    return f"{route.name}_{method}"
```

Requirements for a good operation-ID function:

- deterministic;
- unique across the final application;
- stable across irrelevant refactors;
- valid for downstream code generators.

## 4.13 App state and dependency overrides

Important application attributes include:

```python
app.state
app.dependency_overrides
app.webhooks
```

`app.state` is a generic Starlette state container. Prefer typed wrappers/access dependencies rather than scattering untyped state reads throughout routes.

`dependency_overrides` is principally a test/integration seam; clear overrides after tests.

## 4.14 Constructor anti-patterns

- Setting every possible constructor option “for completeness”.
- Embedding secrets in OpenAPI metadata.
- Setting `debug=True` in production.
- Setting `root_path` when the proxy does not actually strip/add that prefix.
- Disabling `strict_content_type` globally without a client-compatibility reason.
- Using global dependencies for feature-specific behavior.
- Using legacy startup/shutdown event lists for new lifecycle-managed resources.

## 4.15 Agent checklist

```text
[ ] Set title/version/description intentionally for published APIs.
[ ] Decide docs/openapi exposure.
[ ] Use lifespan for shared resource startup/shutdown.
[ ] Leave strict_content_type=True unless compatibility requires otherwise.
[ ] Stabilize operation IDs if generated clients depend on them.
[ ] Configure root_path only for real proxy-prefix topology.
[ ] Avoid global dependencies unless policy is truly global.
```

### Sources

- FastAPI class reference: https://fastapi.tiangolo.com/reference/fastapi/
- Metadata/docs URLs: https://fastapi.tiangolo.com/tutorial/metadata/
- Strict content type: https://fastapi.tiangolo.com/advanced/strict-content-type/
- Behind a proxy: https://fastapi.tiangolo.com/advanced/behind-a-proxy/

---

# FastAPI Advanced — 5) Path operations, decorators, HTTP methods, and operation metadata

## 5.0 Path operation model

A FastAPI path operation combines:

```text
HTTP method + path template + endpoint callable + dependency graph + input schema + output schema + OpenAPI metadata
```

Canonical declaration:

```python
@app.get("/items/{item_id}")
async def read_item(item_id: int) -> ItemRead:
    ...
```

## 5.1 HTTP decorator set

The application/router exposes the normal HTTP methods:

```python
@app.get(...)
@app.post(...)
@app.put(...)
@app.patch(...)
@app.delete(...)
@app.options(...)
@app.head(...)
@app.trace(...)
```

For unusual multi-method registration, lower-level `api_route`/router APIs are available, but explicit method decorators are clearer for ordinary application code.

## 5.2 Dense path-operation configuration shape

Representative route declaration:

```python
@app.post(
    "/items",
    response_model=ItemRead,
    status_code=201,
    tags=["items"],
    summary="Create an item",
    description="Creates one item and returns its public representation.",
    response_description="Created item",
    responses={409: {"description": "Conflict"}},
    deprecated=False,
    operation_id="createItem",
    include_in_schema=True,
)
async def create_item(payload: ItemCreate) -> ItemRead:
    ...
```

FastAPI's decorator API has a broad configuration surface. Treat the Python route declaration as an API contract, not just a handler registration.

## 5.3 Route path syntax

```python
@app.get("/users/{user_id}")
async def get_user(user_id: int):
    ...
```

Path placeholders must correspond to function parameters (directly or via route machinery) and are always required at HTTP routing time. `user_id: int | None = None` does not make a path segment optional.

## 5.4 Route ordering

Static and dynamic paths can overlap:

```python
@app.get("/users/me")
async def me(): ...

@app.get("/users/{user_id}")
async def user(user_id: str): ...
```

Declare more specific routes before a catch-all dynamic route when the router's matching semantics would otherwise capture the static segment.

## 5.5 Path converters / path-like values

For path values that include `/`, FastAPI/Starlette support path-converter syntax, e.g. a path parameter capturing a nested file path. Document it carefully because OpenAPI tooling/browser docs can have different expectations around slash-containing parameters.

## 5.6 Status code as contract

```python
from fastapi import status

@app.post("/items", status_code=status.HTTP_201_CREATED)
```

The decorator status code:

- becomes the default runtime status;
- is documented in OpenAPI;
- may interact with response classes/streaming;
- can be overridden at runtime using a `Response` parameter or explicit `Response` object where appropriate.

Do not return a body with status codes that semantically prohibit bodies unless you intentionally know the HTTP implications.

## 5.7 Tags and summaries

Tags group operations in generated docs. Treat tag names as part of documentation taxonomy, not as authorization roles.

```python
router = APIRouter(prefix="/users", tags=["users"])
```

## 5.8 `name` vs `operation_id`

- route `name` is a Starlette/FastAPI routing identity useful for URL reversing and internal naming;
- OpenAPI `operationId` is the generated-client/API contract identity.

They can be related but are not semantically identical.

## 5.9 `include_in_schema=False`

```python
@app.get("/internal/health", include_in_schema=False)
async def health(): ...
```

This hides the operation from generated OpenAPI. It does **not** disable the route, secure it, or make it inaccessible.

## 5.10 `deprecated=True`

Use for staged API migration:

```python
@app.get("/v1/items", deprecated=True)
```

Clients can see deprecation in OpenAPI, but FastAPI does not automatically enforce a sunset date or emit a deprecation response header. Add those policies explicitly if required.

## 5.11 Additional response declarations

```python
responses={
    404: {"description": "Item not found"},
    409: {"description": "Version conflict"},
}
```

This documents alternatives; it does not magically generate the corresponding runtime branches. Your code must actually return/raise those statuses.

## 5.12 Dependencies at decorator level

```python
@app.delete(
    "/items/{item_id}",
    dependencies=[Depends(require_admin)],
)
async def delete_item(item_id: int):
    ...
```

Use decorator dependencies when the return value is not needed in the handler signature. This can reduce irrelevant parameters while preserving execution/policy.

## 5.13 Return annotation vs `response_model`

Both can define output contracts. Use `response_model=` when:

- the runtime object type differs from the public API model;
- you need output filtering without pretending the handler's Python return type is the public model;
- return annotation should remain a domain/infrastructure type.

Use return annotations naturally when the handler truly returns the response model.

## 5.14 Route declaration anti-patterns

- Unique behavior encoded only in docstrings, not route metadata/schema.
- `include_in_schema=False` as “security”.
- Duplicated route paths/methods with accidental ordering dependence.
- Using route tags as authorization roles.
- Unstable operation IDs generated from module paths that change frequently.
- Returning undocumented status codes everywhere.
- Putting large service implementations directly in decorator functions.

## 5.15 Agent checklist

```text
[ ] Method and path are correct and unambiguous.
[ ] Static-vs-dynamic route ordering is safe.
[ ] status_code matches real HTTP behavior.
[ ] response_model/return annotation reflects public output.
[ ] alternative responses are documented when material.
[ ] operation ID is stable if SDK generation matters.
[ ] include_in_schema is not treated as access control.
[ ] dependencies sit at the narrowest correct scope.
```

### Sources

- FastAPI class/path methods: https://fastapi.tiangolo.com/reference/fastapi/
- APIRouter reference: https://fastapi.tiangolo.com/reference/apirouter/
- Path operation advanced config: https://fastapi.tiangolo.com/advanced/path-operation-advanced-configuration/

---

# FastAPI Advanced — 6) Parameter inference and request-source classification

## 6.0 The inference engine is the core FastAPI abstraction

FastAPI reads the endpoint signature and classifies each parameter. A practical default model is:

```text
Name appears in route path          -> path parameter
Pydantic model / body-shaped type   -> request body (unless explicitly Query/Header/etc.)
Scalar/simple value not in path     -> query parameter by default
Depends/Security                    -> dependency
Request/Response/WebSocket/etc.     -> injected framework object
Explicit Path/Query/Header/Cookie/Body/Form/File -> specified source
```

This is a heuristic summary; explicit parameter helpers are the authoritative way to remove ambiguity.

## 6.1 `Annotated` is the preferred modern declaration style

```python
from typing import Annotated
from fastapi import Query

async def search(
    q: Annotated[str, Query(min_length=2, max_length=100)],
):
    ...
```

Advantages:

- keeps the Python type as the first-class type;
- layers FastAPI/Pydantic metadata without confusing default values;
- composes with reusable type aliases;
- generally improves static type-tool behavior.

## 6.2 Explicit source helpers

```python
from fastapi import Path, Query, Header, Cookie, Body, Form, File
```

Use these when location or constraints are not obvious.

Example:

```python
async def endpoint(
    item_id: Annotated[int, Path(ge=1)],
    q: Annotated[str | None, Query(max_length=50)] = None,
    user_agent: Annotated[str | None, Header()] = None,
):
    ...
```

## 6.3 Required vs optional is a default-value contract

```python
q: str                    # required query value (if inferred query)
q: str | None             # type allows None, but absent/default semantics depend on default
q: str | None = None      # optional; omitted -> None
q: str = "default"        # optional; omitted -> default
```

Do not assume `Optional[T]` alone always means the client may omit the value; Python default presence matters.

## 6.4 Scalar vs model behavior

```python
class Filter(BaseModel):
    q: str | None = None
    limit: int = 20
```

If used as a normal parameter without an explicit source helper in a body-accepting endpoint, a model is typically treated as body data. Modern FastAPI also supports query parameter models using explicit `Query()` annotation patterns.

**Agent rule:** when using a model as a query/header/cookie model, declare the source explicitly. Do not rely on old inference assumptions.

## 6.5 Multiple body parameters

FastAPI can compose multiple body-declared values:

```python
async def update(
    item: Item,
    user: User,
    importance: Annotated[int, Body(gt=0)],
):
    ...
```

The JSON body becomes an object with keys corresponding to the body parameters. This is different from a single Pydantic model body.

## 6.6 `Body(embed=True)`

For a single model you can require a wrapper key:

```python
async def create(item: Annotated[Item, Body(embed=True)]):
    ...
```

Expected JSON:

```json
{"item": {"name": "x", "price": 10}}
```

instead of:

```json
{"name": "x", "price": 10}
```

Use only when the wire contract needs that envelope.

## 6.7 Direct `Request` access bypasses source schema inference

```python
from fastapi import Request

@app.get("/raw")
async def raw(request: Request):
    tenant = request.headers.get("x-tenant")
```

This is valid, but FastAPI cannot automatically infer/validate/document arbitrary values you extract manually. Prefer typed `Header()`/dependency declarations for contract-relevant inputs.

## 6.8 Alias behavior

Parameter helpers can expose a wire name different from Python identifier:

```python
async def endpoint(
    user_agent: Annotated[str | None, Header(alias="User-Agent")] = None,
):
    ...
```

Headers have additional underscore/hyphen conversion conventions. Document aliases when client contract differs from Python names.

## 6.9 Validation metadata families

Common constraints include:

```text
gt / ge / lt / le
min_length / max_length
pattern
multiple_of
strict
examples / openapi_examples
alias / validation_alias / serialization_alias (where supported through Pydantic/FastAPI helpers)
deprecated
include_in_schema
```

Use constraints at the API boundary for syntactic/domain-shape validation, but do not encode every business rule into Pydantic field constraints if it needs database/contextual logic.

## 6.10 Agent source-selection decision table

| Input | Preferred declaration |
|---|---|
| route identifier | `Path()` |
| optional filtering/sorting | `Query()` |
| HTTP metadata | `Header()` |
| browser cookie | `Cookie()` |
| JSON domain payload | Pydantic model / `Body()` |
| HTML form | `Form()` |
| file upload | `UploadFile` / `File()` |
| auth | `Security()` / security helper dependency |
| shared derived context | `Depends()` |
| raw HTTP access | `Request` only when necessary |

## 6.11 Anti-pattern inventory

- Extracting all inputs from `Request` manually.
- Using body models for filter parameters without intending POST/body semantics.
- Assuming `T | None` alone makes omission valid.
- Hiding domain lookup/authorization rules inside field validators that need request context.
- Overusing aliases that make Python and HTTP contracts hard to correlate.
- Relying on implicit source classification for novel/complex generic types without tests.

## 6.12 Agent checklist

```text
[ ] Classify every parameter source.
[ ] Use Annotated + explicit helper where ambiguity exists.
[ ] Confirm requiredness from defaults.
[ ] Keep raw Request access limited.
[ ] Confirm model-as-query/header/cookie uses explicit source.
[ ] Test generated OpenAPI parameter location.
```

### Sources

- Request parameter reference: https://fastapi.tiangolo.com/reference/parameters/
- Query parameters/models: https://fastapi.tiangolo.com/tutorial/query-params/
- Body: https://fastapi.tiangolo.com/tutorial/body/

---

# FastAPI Advanced — 7) Path, query, header, and cookie parameters

## 7.0 `Path(...)`

Path parameters are required because route matching requires a concrete path segment.

```python
from typing import Annotated
from fastapi import Path

@app.get("/items/{item_id}")
async def read_item(
    item_id: Annotated[int, Path(title="Item ID", ge=1)],
):
    return {"item_id": item_id}
```

Constraints are validated after Starlette matches the textual path.

## 7.1 `Query(...)`

```python
@app.get("/items")
async def list_items(
    q: Annotated[str | None, Query(min_length=2, max_length=100)] = None,
    limit: Annotated[int, Query(ge=1, le=100)] = 20,
):
    ...
```

Repeated query keys can map to list-like values:

```python
tag: Annotated[list[str] | None, Query()] = None
```

Test exact encoding expected from client libraries.

## 7.2 Query parameter models

Modern FastAPI can group related query parameters into a Pydantic model with an explicit query declaration. This is valuable for reusable filters/pagination:

```python
class FilterParams(BaseModel):
    limit: int = Field(100, gt=0, le=100)
    offset: int = Field(0, ge=0)
    order_by: Literal["created_at", "updated_at"] = "created_at"

@app.get("/items")
async def list_items(
    filters: Annotated[FilterParams, Query()],
):
    return filters
```

This avoids “parameter soup” while keeping values in the query string rather than moving them into JSON body.

## 7.3 `Header(...)`

```python
@app.get("/items")
async def items(
    if_none_match: Annotated[str | None, Header()] = None,
):
    ...
```

FastAPI commonly converts underscores in Python parameter names to hyphens in header names. If exact naming matters, use an alias.

Headers are case-insensitive at HTTP level; application normalization should not depend on input casing.

## 7.4 Header models

For groups of standard/custom headers, explicit header model support can reduce repetition. Keep model fields aligned with real wire names and verify OpenAPI output.

## 7.5 Duplicate headers

HTTP permits repeated headers in some contexts. Use list-typed header values if your contract intentionally accepts duplicates, and test server/proxy behavior because intermediaries may combine or normalize headers.

## 7.6 `Cookie(...)`

```python
@app.get("/session")
async def session(
    session_id: Annotated[str | None, Cookie()] = None,
):
    ...
```

Reading a cookie is not the same as creating/setting one. Set response cookies using a `Response` object or an explicit response instance.

## 7.7 Cookie models

Grouping cookies in a Pydantic model can be appropriate for a coherent cookie contract. Avoid putting a large browser session mechanism into plain unsigned cookies merely because parsing is convenient.

## 7.8 Enum and Literal values

Use `Enum` or `Literal` for closed sets:

```python
Sort = Literal["name", "created_at", "price"]

@app.get("/items")
async def list_items(sort: Sort = "name"):
    ...
```

This improves runtime validation, editor completion, and OpenAPI schema.

## 7.9 UUID/date/time/decimal and extra types

FastAPI/Pydantic can parse many standard/Pydantic-supported types directly. Example:

```python
from datetime import date
from uuid import UUID

@app.get("/reports/{report_id}")
async def report(
    report_id: UUID,
    as_of: date,
):
    ...
```

Prefer domain-appropriate typed values at the boundary instead of strings parsed again in service logic.

## 7.10 Strict vs coercive validation

Pydantic often coerces compatible values unless strict constraints/types are used. API design should decide where coercion is desirable. For example, query strings are text by nature, so converting `"10"` to `int` is expected; JSON body coercion may deserve stricter policy for some fields.

## 7.11 Parameter metadata and deprecation

```python
old: Annotated[str | None, Query(deprecated=True)] = None
```

This marks OpenAPI metadata. It does not stop callers from sending the parameter.

## 7.12 Parameter anti-patterns

- Query bodies for ordinary GET filters when query parameters are the real contract.
- Unbounded `limit` values.
- Header-based authorization implemented manually in every route rather than a dependency/security scheme.
- Cookie authentication without `Secure`, `HttpOnly`, `SameSite`, CSRF design, and rotation policy.
- Parsing UUID/date/enums manually from strings after FastAPI already supports typed parsing.

## 7.13 Agent checklist

```text
[ ] Path params are required and constrained.
[ ] Query pagination has bounds.
[ ] Closed sets use Literal/Enum.
[ ] Header aliases/conversion are understood.
[ ] Cookie parsing is separated from cookie security policy.
[ ] Typed standard values are used when possible.
[ ] Parameter model OpenAPI matches intended wire source.
```

### Sources

- Path parameters: https://fastapi.tiangolo.com/tutorial/path-params/
- Query parameters: https://fastapi.tiangolo.com/tutorial/query-params/
- Header parameters: https://fastapi.tiangolo.com/tutorial/header-params/
- Cookie parameters: https://fastapi.tiangolo.com/tutorial/cookie-params/
- Parameter reference: https://fastapi.tiangolo.com/reference/parameters/

---

# FastAPI Advanced — 8) Request bodies, Pydantic models, nesting, examples, and schema behavior

## 8.0 Body model as API boundary

```python
from pydantic import BaseModel, Field

class ItemCreate(BaseModel):
    name: str = Field(min_length=1, max_length=200)
    price: float = Field(gt=0)
    description: str | None = None

@app.post("/items")
async def create_item(item: ItemCreate):
    ...
```

FastAPI reads JSON, validates it into `ItemCreate`, and uses the model's JSON Schema in OpenAPI.

## 8.1 Nested models

```python
class Image(BaseModel):
    url: str
    name: str

class Item(BaseModel):
    name: str
    images: list[Image] = []
```

Avoid mutable literal defaults in ordinary Python dataclasses; Pydantic handles model defaults specially, but `Field(default_factory=list)` communicates intent more clearly:

```python
images: list[Image] = Field(default_factory=list)
```

## 8.2 Deep containers and unions

```python
class Payload(BaseModel):
    labels: dict[str, str]
    points: list[tuple[float, float]]
    mode: Literal["fast", "accurate"]
```

Schema complexity affects generated clients. Do not create deeply polymorphic unions simply because Pydantic can represent them if downstream OpenAPI tooling cannot consume them reliably.

## 8.3 Discriminated unions

When a field can contain variants, use Pydantic's discriminated-union support so the wire contract has an explicit tag. This is generally better than ambiguous shape-based unions for generated clients.

## 8.4 Multiple body values

```python
@app.put("/items/{item_id}")
async def update_item(
    item_id: int,
    item: Item,
    user: User,
    importance: Annotated[int, Body(gt=0)],
):
    ...
```

Wire shape:

```json
{
  "item": {...},
  "user": {...},
  "importance": 5
}
```

## 8.5 Singular body values

Use `Body()` for non-model JSON body values when appropriate:

```python
importance: Annotated[int, Body(gt=0)]
```

Without explicit `Body`, a scalar would normally be inferred as a query parameter.

## 8.6 Examples

Examples can be declared through Pydantic schema metadata or FastAPI parameter/body metadata. Prefer examples that demonstrate boundary cases and valid domain shapes, not secrets or production identifiers.

```python
class Item(BaseModel):
    model_config = {
        "json_schema_extra": {
            "examples": [
                {"name": "Widget", "price": 12.5}
            ]
        }
    }
```

## 8.7 Separate input/output schemas

With `separate_input_output_schemas=True`, the OpenAPI schema may represent a model differently for validation vs serialization. This can matter when defaults and serialization behavior imply different required fields.

Contract-testing advice:

```text
Do not assume one Pydantic class name maps to one JSON Schema component under every FastAPI/Pydantic configuration.
Snapshot the actual OpenAPI output used by clients.
```

## 8.8 PATCH/update models

A common partial-update pattern:

```python
class ItemPatch(BaseModel):
    name: str | None = None
    price: float | None = None

@app.patch("/items/{item_id}")
async def patch_item(item_id: int, patch: ItemPatch):
    changes = patch.model_dump(exclude_unset=True)
    ...
```

`exclude_unset=True` distinguishes fields omitted by the client from fields explicitly provided with default/`None` values.

## 8.9 `jsonable_encoder`

`jsonable_encoder` converts objects into JSON-compatible Python values. It is useful when manually constructing a `JSONResponse` or storing model-like data in JSON-oriented systems.

```python
from fastapi.encoders import jsonable_encoder

payload = jsonable_encoder(model)
```

When returning normal Python/Pydantic values through FastAPI's standard response path, you often do not need to call it manually.

## 8.10 Strict content type for JSON bodies

FastAPI 0.141.1 defaults `strict_content_type=True`. A JSON body without an appropriate Content-Type is rejected instead of being opportunistically parsed as JSON. Preserve this unless supporting a known legacy client contract.

## 8.11 Body size and resource limits

Pydantic field validation does not replace infrastructure-level request-body limits. For large payloads/uploads, configure proxy/server/application limits and consider streaming rather than materializing huge JSON structures.

## 8.12 Body-model anti-patterns

- Reusing ORM/internal models as public API contracts without reviewing exposed fields.
- One enormous “everything optional” model for create/read/update.
- Ambiguous unions that SDK generators cannot model.
- Calling `model_dump()` everywhere inside routes instead of passing typed objects into services.
- Treating Pydantic validation as database uniqueness/authorization validation.
- Disabling strict Content-Type to mask broken client behavior without analysis.

## 8.13 Agent checklist

```text
[ ] Use distinct input/output/patch models when semantics differ.
[ ] Prefer nested typed models over raw dict[str, Any].
[ ] Use discriminators for polymorphic payloads.
[ ] Verify multiple-body wire shape.
[ ] Use exclude_unset for PATCH semantics.
[ ] Snapshot actual OpenAPI input/output schemas.
[ ] Keep strict_content_type enabled by default.
```

### Sources

- Request body: https://fastapi.tiangolo.com/tutorial/body/
- Nested models: https://fastapi.tiangolo.com/tutorial/body-nested-models/
- Multiple body params: https://fastapi.tiangolo.com/tutorial/body-multiple-params/
- Body updates: https://fastapi.tiangolo.com/tutorial/body-updates/
- `jsonable_encoder`: https://fastapi.tiangolo.com/reference/encoders/

---

# FastAPI Advanced — 9) Forms, multipart requests, files, and `UploadFile`

## 9.0 Dependency requirement

Form/multipart parsing requires `python-multipart`; it is included by `fastapi[standard]`.

## 9.1 Form fields

```python
from typing import Annotated
from fastapi import Form

@app.post("/login")
async def login(
    username: Annotated[str, Form()],
    password: Annotated[str, Form()],
):
    ...
```

Form data uses form media types, not JSON. A client sending JSON to this endpoint does not satisfy the same contract.

## 9.2 Form models

FastAPI supports Pydantic models for grouped form fields in modern versions. This can provide the same reuse/validation benefits as body models while preserving form wire format.

## 9.3 Raw file bytes

```python
from fastapi import File

@app.post("/files")
async def upload(file: Annotated[bytes, File()]):
    return {"size": len(file)}
```

This materializes the entire file into memory. Use only for known-small files.

## 9.4 `UploadFile`

```python
from fastapi import UploadFile

@app.post("/files")
async def upload(file: UploadFile):
    content_type = file.content_type
    filename = file.filename
    chunk = await file.read(64 * 1024)
    ...
```

`UploadFile` is preferable for potentially larger uploads because it uses a spooled file abstraction rather than forcing full bytes into one in-memory value.

Useful methods/properties include:

```text
filename
content_type
headers
file
read(size)
write(data)
seek(offset)
close()
```

## 9.5 Multiple files

```python
@app.post("/files")
async def upload(files: list[UploadFile]):
    ...
```

Define count/size policy outside the type alone; a list annotation does not enforce a practical upload quota.

## 9.6 Forms and files together

Multipart can combine fields and file parts:

```python
@app.post("/documents")
async def create_document(
    title: Annotated[str, Form()],
    file: UploadFile,
):
    ...
```

Do not expect an additional JSON body simultaneously in the same request; multipart is the body encoding.

## 9.7 File security controls

Application policy should address:

- maximum request/file size;
- maximum file count;
- content-type allowlist as a hint, not proof;
- magic-byte/content inspection if needed;
- filename normalization; never trust client filename as filesystem path;
- malware scanning for relevant environments;
- storage outside web root;
- generated object keys rather than raw filenames;
- decompression bomb protections;
- timeout/rate limits.

## 9.8 Streaming upload to storage

Avoid:

```python
data = await file.read()  # entire unbounded file
await object_store.put(data)
```

Prefer bounded chunks where the storage client supports streaming:

```python
while chunk := await file.read(1024 * 1024):
    await writer.write(chunk)
```

Remember: the ASGI server/proxy may already buffer parts depending on stack configuration. Measure actual memory behavior.

## 9.9 `UploadFile.file` and sync libraries

`UploadFile.file` exposes the underlying file-like object, useful with synchronous libraries that expect file handles. If called inside `async def`, be aware of blocking behavior in downstream libraries.

## 9.10 Anti-pattern inventory

- `bytes` for multi-gigabyte uploads.
- Trusting `content_type` as security validation.
- Saving to `uploads/{file.filename}` directly.
- No upload count/size limits.
- Reading the entire file merely to forward it to another service.
- Accepting archive files without decompression limits.
- Running CPU-heavy parsing/virus scanning directly on event loop.

## 9.11 Agent checklist

```text
[ ] python-multipart installed.
[ ] Use UploadFile for nontrivial uploads.
[ ] Bound file count and size.
[ ] Sanitize/generate storage names.
[ ] Treat MIME/filename as untrusted metadata.
[ ] Stream to storage when possible.
[ ] Offload blocking/CPU-heavy file processing.
```

### Sources

- Forms: https://fastapi.tiangolo.com/tutorial/request-forms/
- Form models: https://fastapi.tiangolo.com/tutorial/request-form-models/
- Files: https://fastapi.tiangolo.com/tutorial/request-files/
- Forms/files: https://fastapi.tiangolo.com/tutorial/request-forms-and-files/
- UploadFile reference: https://fastapi.tiangolo.com/reference/uploadfile/

---

# FastAPI Advanced — 10) Response models, return annotations, filtering, and serialization

## 10.0 Response model is an outbound trust boundary

Input validation protects the application from malformed client data. **Response validation/filtering protects the client contract from accidental server data exposure.**

```python
class UserInternal(BaseModel):
    id: int
    email: str
    password_hash: str

class UserPublic(BaseModel):
    id: int
    email: str

@app.get("/users/{user_id}", response_model=UserPublic)
async def user(user_id: int):
    return UserInternal(...)
```

The public response model prevents `password_hash` from being emitted through the normal FastAPI serialization path.

## 10.1 Return annotation as response contract

```python
@app.get("/items/{item_id}")
async def item(item_id: int) -> ItemRead:
    return await service.get(item_id)
```

FastAPI can use the annotation for response schema/validation. This is ideal when the Python return type genuinely is the public response model.

## 10.2 Explicit `response_model`

```python
@app.get("/items/{item_id}", response_model=ItemRead)
async def item(item_id: int) -> DomainItem:
    return await service.get(item_id)
```

This separates Python implementation type from wire contract. It is often the best pattern when routes return ORM/domain objects.

## 10.3 Output filtering controls

Path decorators expose response-model controls such as inclusion/exclusion and unset/default/none behavior. Prefer defining a dedicated response model over a large ad hoc include/exclude set when the public shape is stable.

Examples of intent:

```text
response_model_exclude_unset=True  -> omit fields never set
response_model_exclude_defaults=True -> omit values equal to defaults
response_model_exclude_none=True -> omit None values
```

These alter wire semantics; generated schemas may not fully communicate every runtime omission nuance. Use consistently.

## 10.4 Lists and containers

```python
@app.get("/items")
async def items() -> list[ItemRead]:
    ...
```

Container return types can generate structured response schemas. Recent 0.140.x fixes addressed handling of iterable/generator-adjacent return types; pinning 0.141.1 avoids known pre-fix behavior.

## 10.5 `None` and optional responses

```python
@app.get("/maybe")
async def maybe() -> ItemRead | None:
    ...
```

Be explicit about whether `null` is a successful 200 payload or whether absence should be a 404/204. HTTP semantics should drive the contract, not only Python typing.

## 10.6 `Response` return bypass

```python
from fastapi.responses import JSONResponse

@app.get("/raw")
async def raw():
    return JSONResponse({"ok": True})
```

When returning a `Response` subclass directly, FastAPI passes it through and does not apply normal Pydantic response-model conversion. This is intentional full control.

## 10.7 `jsonable_encoder` for manual responses

```python
payload = jsonable_encoder(model)
return JSONResponse(payload)
```

Use when manually constructing a response from values containing datetime/UUID/Pydantic objects. If you simply return the model through normal FastAPI handling, manual encoding is usually unnecessary.

## 10.8 ORM/domain serialization

Do not return arbitrary ORM objects and assume every private relationship/attribute is invisible. Define explicit API schema models and test serialized output.

With Pydantic v2, model configuration such as attribute-based validation can bridge domain/ORM objects into API models. Keep the public schema separate from persistence internals.

## 10.9 Response validation failures

If the application returns data inconsistent with its declared response model, that is normally a server bug, not a client error. Treat response validation exceptions as defects and surface them in logs/observability rather than returning a misleading 422 to the caller.

## 10.10 Serialization performance

Response validation/serialization has cost. Strategies:

- keep response models precise but not absurdly nested;
- paginate large collections;
- stream huge datasets instead of building giant lists;
- benchmark alternative JSON response classes only after profiling;
- avoid converting the same large object through multiple representation layers.

Correctness and contract safety generally outweigh micro-optimization of serialization.

## 10.11 Anti-pattern inventory

- Returning internal models with secrets and no response model.
- Declaring `dict[str, Any]` for everything to avoid schema work.
- Direct `JSONResponse` everywhere, bypassing response models.
- Using 200 + `null` for every not-found case without intentional API semantics.
- Massive unpaginated list responses.
- Treating response validation errors as user input errors.

## 10.12 Agent checklist

```text
[ ] Public response model excludes internal fields.
[ ] response_model vs return annotation chosen intentionally.
[ ] Optional/null vs 404/204 semantics decided.
[ ] Direct Response only when bypass is intended.
[ ] Large collections paginated or streamed.
[ ] Response validation failures monitored as server defects.
```

### Sources

- Response model: https://fastapi.tiangolo.com/tutorial/response-model/
- Return Response directly: https://fastapi.tiangolo.com/advanced/response-directly/
- Encoders: https://fastapi.tiangolo.com/reference/encoders/

---
# FastAPI Advanced — 11) Response classes, status codes, cookies, headers, files, and redirects

## 11.0 Two response modes

FastAPI has two fundamentally different response paths:

```text
A. return normal Python value
   -> response model / encoder / default response class

B. return Response instance
   -> response is passed through directly
```

Choose B only when you actually need protocol-level control.

## 11.1 `Response` parameter for mutation without bypassing serialization

You can receive a `Response` object, mutate headers/cookies/status, and still return a normal value that goes through the response model:

```python
from fastapi import Response

@app.get("/items/{item_id}", response_model=ItemRead)
async def get_item(item_id: int, response: Response) -> ItemRead:
    response.headers["X-Resource-Version"] = "7"
    return await service.get(item_id)
```

This is often superior to manually constructing `JSONResponse` because it preserves output validation/serialization.

## 11.2 Runtime status-code mutation

```python
@app.put("/items/{item_id}", response_model=ItemRead)
async def upsert_item(item_id: int, response: Response) -> ItemRead:
    item, created = await service.upsert(item_id)
    if created:
        response.status_code = 201
    return item
```

Document all meaningful alternative statuses in `responses={...}` if clients need the contract.

## 11.3 Core response classes

Common FastAPI/Starlette response classes:

| Class | Primary use |
|---|---|
| `Response` | raw bytes/text with manual media type |
| `JSONResponse` | normal JSON response |
| `ORJSONResponse` | optional high-performance JSON via `orjson` |
| `UJSONResponse` | optional `ujson` path; prefer only with explicit need |
| `HTMLResponse` | HTML text |
| `PlainTextResponse` | plain text |
| `RedirectResponse` | HTTP redirect |
| `FileResponse` | filesystem-backed file response |
| `StreamingResponse` | generic chunked/streaming body |
| `EventSourceResponse` | FastAPI SSE stream |

## 11.4 Custom default response class

```python
app = FastAPI(default_response_class=ORJSONResponse)
```

Route-specific override:

```python
@app.get("/health", response_class=PlainTextResponse)
async def health() -> str:
    return "ok"
```

`response_class=` also influences OpenAPI media-type documentation. Returning an arbitrary explicit `Response` can diverge from documented media type unless you configure both coherently.

## 11.5 `HTMLResponse`

```python
@app.get("/about", response_class=HTMLResponse)
async def about() -> str:
    return "<h1>About</h1>"
```

Do not interpolate untrusted input directly into HTML. Use a templating/escaping strategy for dynamic pages.

## 11.6 Redirects

```python
from fastapi.responses import RedirectResponse

@app.get("/old")
async def old():
    return RedirectResponse("/new", status_code=307)
```

Select redirect status intentionally:

- `301`/`308` for permanent semantics;
- `302`/`307` for temporary semantics;
- `307`/`308` preserve method/body semantics more explicitly than historical `302` behavior.

## 11.7 `FileResponse`

```python
from fastapi.responses import FileResponse

@app.get("/reports/{name}")
async def report(name: str):
    path = resolve_allowed_report(name)
    return FileResponse(path, filename=f"{name}.pdf")
```

Security rules:

- never concatenate raw user paths into filesystem paths;
- enforce an allowlisted root;
- protect authorization separately;
- set content-disposition intentionally;
- consider object storage / signed URLs for very large files or CDN workloads.

## 11.8 Cookies

```python
@app.post("/session")
async def create_session(response: Response):
    response.set_cookie(
        "session",
        "opaque-token",
        httponly=True,
        secure=True,
        samesite="lax",
        max_age=3600,
    )
    return {"ok": True}
```

Cookie auth requires a broader design: CSRF defenses, rotation/revocation, path/domain policy, expiry, HTTPS, secret storage, and browser behavior.

Delete cookies with matching path/domain attributes:

```python
response.delete_cookie("session")
```

## 11.9 Response headers

```python
response.headers["Cache-Control"] = "private, max-age=60"
response.headers["ETag"] = '"abc123"'
```

Global headers belong in middleware when they apply to nearly every response. Route-specific cache/ETag/content-disposition headers can be set at the response layer.

## 11.10 Conditional requests

FastAPI does not automatically implement application-level ETag/version semantics. If you expose `ETag`, `If-Match`, `If-None-Match`, or `Last-Modified`, implement the corresponding conditional behavior coherently rather than emitting decorative headers.

## 11.11 No-content responses

For `204 No Content`, return no body. Avoid Pydantic/body models suggesting a JSON payload when HTTP semantics say otherwise.

## 11.12 Response anti-patterns

- `JSONResponse` in every route, bypassing response-model safety.
- Setting global security headers manually in every endpoint.
- `FileResponse` over user-controlled arbitrary paths.
- Cookie auth without `Secure`/`HttpOnly`/CSRF analysis.
- Mismatched documented `response_class` and actual explicit response.
- Returning bodies with 204 status.

## 11.13 Agent checklist

```text
[ ] Prefer Response parameter for header/cookie/status mutation when preserving response model.
[ ] Return explicit Response only for full control.
[ ] Match media type and docs.
[ ] Protect file paths and downloads.
[ ] Set cookie security attributes deliberately.
[ ] Document alternative status codes.
```

### Sources

- Custom responses: https://fastapi.tiangolo.com/advanced/custom-response/
- Response reference: https://fastapi.tiangolo.com/reference/response/
- Response cookies: https://fastapi.tiangolo.com/advanced/response-cookies/
- Response headers: https://fastapi.tiangolo.com/advanced/response-headers/

---

# FastAPI Advanced — 12) Streaming responses and JSON Lines

## 12.0 Streaming families

FastAPI 0.141.1 has multiple streaming mechanisms. Do not collapse them into one concept:

| Mechanism | Wire format | Best use |
|---|---|---|
| generator return with typed iterable | JSON Lines (`application/jsonl`) | structured sequence of JSON items |
| `StreamingResponse` | arbitrary bytes/chunks/media type | files, proxying, binary/text custom streams |
| `EventSourceResponse` | SSE (`text/event-stream`) | browser-native server push / event streams |

JSON Lines support was added in FastAPI **0.134.0**.

## 12.1 First-class JSON Lines via `yield`

```python
from collections.abc import AsyncIterable
from pydantic import BaseModel

class Item(BaseModel):
    name: str
    description: str | None = None

@app.get("/items/stream")
async def stream_items() -> AsyncIterable[Item]:
    async for item in repository.iter_items():
        yield item
```

FastAPI recognizes the generator/iterable return and streams one JSON object per line as `application/jsonl`.

## 12.2 Return annotation matters

The typed iterable tells FastAPI the **item type**:

```python
AsyncIterable[Item]
```

FastAPI can use it for:

- per-item Pydantic validation/serialization;
- OpenAPI documentation;
- efficient serialization paths.

The official docs note a performance advantage when the return type is declared because Pydantic can serialize through its optimized implementation.

## 12.3 Synchronous generator JSONL

```python
from collections.abc import Iterable

@app.get("/items/stream-sync")
def stream_items_sync() -> Iterable[Item]:
    for item in blocking_source():
        yield item
```

FastAPI handles normal generator functions appropriately so they do not run directly as a long blocking loop on the event loop.

## 12.4 No return annotation

A generator can stream without a return annotation, but you lose some schema/validation/serialization information. For an API contract, prefer typed iterables.

## 12.5 JSON Lines vs JSON array

JSON array:

```json
[{"id":1},{"id":2},{"id":3}]
```

requires the client to interpret one complete JSON document; many serializers build/close the collection as a unit.

JSON Lines:

```text
{"id":1}\n
{"id":2}\n
{"id":3}\n
```

lets the producer and consumer process items incrementally. It is particularly useful for:

- large query results;
- AI/LLM structured outputs;
- logs/telemetry;
- export pipelines;
- indefinite sequences where each item is JSON-shaped.

## 12.6 Generic `StreamingResponse`

Use when the chunks are not individual JSON objects:

```python
from fastapi.responses import StreamingResponse

async def byte_stream():
    async for chunk in object_store.stream("large.bin"):
        yield chunk

@app.get("/download")
async def download():
    return StreamingResponse(
        byte_stream(),
        media_type="application/octet-stream",
    )
```

When returning `StreamingResponse` directly, you own chunk encoding and much of the response contract.

## 12.7 Cancellation and disconnects

Streaming endpoints can outlive normal short requests. Design for:

- client disconnect/cancellation;
- upstream cancellation;
- resource cleanup in `finally`/context managers;
- bounded buffering;
- proxy timeout/buffering behavior;
- backpressure from slow clients.

Do not spawn a producer that continues expensive work after the client is gone unless the work is intentionally detached/durable.

## 12.8 Streaming and dependency scopes

A request-scoped `yield` dependency may remain open through the response cycle. If a DB session is needed to lazily produce the stream, that may be necessary.

If a dependency is needed only before the handler and not during streaming, `Depends(scope="function")` can close it after the path operation returns but before the response is sent. See section 15.

This distinction prevents a slow client from unnecessarily holding scarce resources.

## 12.9 Proxy buffering

Even if FastAPI yields incrementally, an upstream proxy can buffer the response and destroy perceived streaming. Validate end-to-end behavior in the deployed topology, not just with in-process tests.

## 12.10 Stream item errors

Once HTTP headers/body streaming has begun, changing the response status may no longer be possible in the ordinary way. Validate predictable failures before starting the stream when possible. Define an in-band error/event record if the protocol needs to represent later failures.

## 12.11 0.140.x fixes

The 0.140.x line included fixes for streaming return-model metadata and status handling, including:

- preservation of stream item type through `include_router()`;
- correct `status_code` behavior for SSE and JSONL endpoints;
- iterable/response-model edge cases.

Targeting 0.141.1 incorporates those fixes.

## 12.12 Streaming anti-patterns

- Building a huge list then yielding from it — memory benefit already lost.
- Using JSONL when clients require one valid JSON array/document.
- Holding a DB transaction for minutes because a slow client consumes data slowly.
- Assuming local streaming means proxy/CDN streaming.
- Blocking synchronous work in an async generator.
- Trying to change status after the stream has already begun.

## 12.13 Agent checklist

```text
[ ] Choose JSONL vs StreamingResponse vs SSE intentionally.
[ ] Type JSONL generator as AsyncIterable[T] or Iterable[T].
[ ] Bound buffering.
[ ] Handle cancellation/disconnect cleanup.
[ ] Decide dependency scope for stream lifetime.
[ ] Verify proxy buffering/timeouts.
[ ] Test late producer failures.
```

### Sources

- JSON Lines: https://fastapi.tiangolo.com/tutorial/stream-json-lines/
- Stream data: https://fastapi.tiangolo.com/advanced/stream-data/
- Custom responses: https://fastapi.tiangolo.com/advanced/custom-response/
- Release notes: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 13) Server-Sent Events (SSE)

## 13.0 Version anchor

First-class SSE support was added in FastAPI **0.135.0**. In 0.141.1, use `fastapi.sse.EventSourceResponse` / `ServerSentEvent` rather than defaulting to older third-party snippets unless a third-party library adds a feature you specifically need.

## 13.1 Minimal typed SSE endpoint

```python
from collections.abc import AsyncIterable
from fastapi import FastAPI
from fastapi.sse import EventSourceResponse
from pydantic import BaseModel

app = FastAPI()

class EventPayload(BaseModel):
    message: str
    sequence: int

@app.get("/events", response_class=EventSourceResponse)
async def events() -> AsyncIterable[EventPayload]:
    for i in range(10):
        yield EventPayload(message="update", sequence=i)
```

Each yielded object is JSON-encoded into the SSE `data:` field. The typed return can drive validation/documentation/serialization.

## 13.2 SSE wire format

Conceptually:

```text
id: 123
event: update
data: {"message":"hello"}
retry: 5000

```

Blank lines terminate events. Browsers support SSE natively through `EventSource` for GET-based streams.

## 13.3 `ServerSentEvent`

Use the explicit event object when you need event metadata rather than only JSON `data`:

```python
from fastapi.sse import ServerSentEvent

async def stream():
    yield ServerSentEvent(
        data={"message": "ready"},
        event="status",
        id="42",
        retry=5000,
    )
```

Consult the 0.141.1 `fastapi.sse` reference for exact accepted data types/arguments; keep custom event formatting out of route code where possible.

## 13.4 Raw data

SSE can carry text/raw event data when appropriate. For structured application streams, JSON/Pydantic payloads are easier to evolve and test.

## 13.5 Resumption with `Last-Event-ID`

A reconnecting SSE client can send `Last-Event-ID`. Implement resumability only when the backend has a replayable event log or cursor. An in-memory generator cannot promise durable resume semantics merely because IDs are present.

Pattern:

```python
async def events(last_event_id: Annotated[str | None, Header()] = None):
    cursor = decode_cursor(last_event_id)
    async for record in event_log.after(cursor):
        yield ServerSentEvent(id=record.id, data=record.payload)
```

## 13.6 SSE with POST

FastAPI's SSE support can be used with methods beyond browser `EventSource` GET patterns, but standard browser `EventSource` itself is GET-oriented. For POST-streaming clients, use `fetch`/HTTP client behavior and define the protocol deliberately.

## 13.7 Keepalives and infrastructure

Long-lived SSE connections require deployment tuning:

- reverse proxy read/idle timeout;
- response buffering disabled where required;
- server worker connection capacity;
- load balancer idle timeout;
- heartbeat/comment strategy if infrastructure closes idle streams;
- graceful shutdown behavior;
- client reconnect policy.

## 13.8 SSE vs WebSocket vs JSONL

| Requirement | Prefer |
|---|---|
| server → browser event push over HTTP | SSE |
| bidirectional interactive messages | WebSocket |
| structured sequential API response | JSON Lines |
| binary chunks/media | `StreamingResponse` / specialized protocol |
| durable pub/sub | external broker + SSE/WebSocket delivery layer |

## 13.9 SSE authorization

Browser `EventSource` has constraints around custom headers. Cookie-based auth or signed URL/token query designs may appear, but each has security tradeoffs. For non-browser SSE clients, normal Authorization headers are simpler.

Never put long-lived bearer secrets in URLs unless the full logging/referrer/cache exposure model is acceptable.

## 13.10 SSE anti-patterns

- Using SSE for client→server interactive command streams.
- Event IDs with no replay backend but claiming guaranteed resume.
- No proxy timeout/buffering tests.
- Holding scarce DB transactions for the full connection.
- No disconnect/cancellation handling.
- Logging full authenticated SSE URLs containing tokens.

## 13.11 Agent checklist

```text
[ ] Use EventSourceResponse from fastapi.sse.
[ ] Type yielded payloads where possible.
[ ] Use ServerSentEvent for id/event/retry metadata.
[ ] Decide whether resumption is real or best-effort.
[ ] Tune proxy/load-balancer idle behavior.
[ ] Keep long-lived resource usage bounded.
[ ] Use WebSocket if true bidirectionality is required.
```

### Sources

- SSE tutorial: https://fastapi.tiangolo.com/tutorial/server-sent-events/
- SSE reference: https://fastapi.tiangolo.com/reference/sse/
- Release notes: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 14) Dependency injection fundamentals

## 14.0 Dependency model

A dependency is a callable FastAPI resolves as part of the request graph.

```python
Depends(dependency=None, *, use_cache=True, scope=None)
```

The dependency itself can have normal FastAPI parameters and other dependencies.

## 14.1 Basic dependency

```python
from typing import Annotated
from fastapi import Depends

async def pagination(skip: int = 0, limit: int = 100) -> tuple[int, int]:
    return skip, limit

Pagination = Annotated[tuple[int, int], Depends(pagination)]

@app.get("/items")
async def items(page: Pagination):
    ...
```

FastAPI resolves `skip` and `limit` as query parameters for the dependency, then injects the returned tuple into the endpoint.

## 14.2 Dependency graph

```text
route
├─ current_user
│  ├─ token
│  └─ user_repository
│     └─ db_session
└─ service
   ├─ user_repository
   │  └─ db_session
   └─ audit_logger
```

With default caching, repeated equivalent dependencies are generally resolved once per request and reused, preventing duplicate `db_session` construction in common graphs.

## 14.3 `use_cache=True`

Default behavior:

```python
Depends(get_db)  # use_cache=True
```

After first resolution in a request, reuse the value where the same dependency is needed again in the dependency graph.

Disable only when repeated execution is semantically required:

```python
Depends(now_value, use_cache=False)
```

Do not disable caching just to “be safe”; it can duplicate expensive auth/DB work and alter identity/resource semantics.

## 14.4 Dependencies without injected return values

Decorator-level:

```python
@router.get(
    "/admin",
    dependencies=[Depends(require_admin)],
)
async def admin():
    ...
```

Useful for guards/audit prerequisites whose values are not needed by the handler.

## 14.5 Class dependencies

A class can be a callable dependency or be instantiated by dependency injection based on its constructor signature.

```python
class Pagination:
    def __init__(self, skip: int = 0, limit: int = 100):
        self.skip = skip
        self.limit = limit

@app.get("/items")
async def items(p: Annotated[Pagination, Depends()]):
    ...
```

Use classes when they improve cohesion; do not turn every two-parameter dependency into an object.

## 14.6 Reusable `Annotated` aliases

```python
CurrentUser = Annotated[User, Depends(get_current_user)]
DbSession = Annotated[Session, Depends(get_db)]
```

This can make route signatures concise while preserving type information. Name aliases clearly so the hidden dependency is discoverable.

## 14.7 Global, router, route, and parameter dependency scopes

Policy can be attached at multiple levels:

```text
FastAPI(dependencies=[...])        -> application-wide
APIRouter(dependencies=[...])      -> router subtree
include_router(... dependencies)   -> composed inclusion context
@app.get(... dependencies=[...])   -> one operation, no injected value
parameter Depends(...)             -> one operation and injected value
```

Use the narrowest scope that matches the policy.

## 14.8 Dependencies as integration seams

Good dependency uses:

- current principal / tenant;
- request-scoped DB session/transaction;
- service construction from repositories/clients;
- feature flags/config-derived context;
- idempotency key validation;
- correlation metadata.

Poor uses:

- hiding arbitrary domain side effects;
- long background workflows;
- stateful singletons recreated per request;
- every pure helper function.

## 14.9 Dependency signatures are part of OpenAPI

If a dependency declares query/header/cookie/security inputs, those can surface on dependent path operations in generated OpenAPI. Changing a shared dependency can therefore change many API operations simultaneously.

Contract tests should catch that blast radius.

## 14.10 Dependency exceptions

Dependencies can raise `HTTPException` or application exceptions before the handler executes. This is useful for auth/lookup preconditions, but centralize semantics rather than raising arbitrary statuses from deeply nested generic infrastructure.

## 14.11 Dependency anti-patterns

- `use_cache=False` everywhere.
- Global dependencies that perform database writes.
- Dependencies that create pools/clients per request.
- Circular sub-dependencies.
- Hidden network calls in trivial-looking aliases.
- Business workflows represented as dependencies because they execute “before the route”.
- Changing shared dependency parameters without reviewing OpenAPI changes across all consumers.

## 14.12 Agent checklist

```text
[ ] Draw the dependency graph for complex routes.
[ ] Keep default per-request caching unless repeated execution is needed.
[ ] Place policy at narrowest correct app/router/route/parameter scope.
[ ] Use aliases for clarity, not opacity.
[ ] Keep shared resources in lifespan; request handles/sessions in dependencies.
[ ] Review OpenAPI impact of shared dependency changes.
```

### Sources

- Dependencies tutorial: https://fastapi.tiangolo.com/tutorial/dependencies/
- Depends reference: https://fastapi.tiangolo.com/reference/dependencies/
- Sub-dependencies: https://fastapi.tiangolo.com/tutorial/dependencies/sub-dependencies/

---

# FastAPI Advanced — 15) Advanced dependencies: sub-dependencies, caching, `yield`, scope, and overrides

## 15.0 Current `Depends` signature

For FastAPI 0.141.1:

```python
Depends(
    dependency=None,
    *,
    use_cache=True,
    scope=None,
)
```

`scope` is especially important for dependencies that use `yield`.

## 15.1 `yield` dependency as request context manager

```python
async def get_session():
    session = Session()
    try:
        yield session
    finally:
        await session.close()
```

The yielded value is injected; code after `yield` is cleanup. FastAPI handles nested `yield` dependencies in correct teardown order.

## 15.2 Exactly one yield

Treat the dependency as a context manager. A dependency intended for this mode should yield once; multiple yields do not model FastAPI dependency lifetime correctly.

## 15.3 `scope="request"`

Conceptually:

```text
enter dependency
→ resolve/execute route
→ create/send response (including response cycle)
→ exit dependency
```

Use when the yielded resource must remain valid through response sending/streaming.

Example: a lazy result stream that still reads from a DB cursor/session.

## 15.4 `scope="function"`

Conceptually:

```text
enter dependency
→ execute path operation function
→ exit dependency
→ send response
```

Use when the resource is needed only to compute/authorize the result and should be released before a potentially slow response stream is transmitted.

```python
async def verify_user(
    session: Annotated[Session, Depends(get_session, scope="function")],
):
    ...
```

This feature was added before the 0.141 line and is now part of the current reference surface.

## 15.5 Sub-dependency scope constraint

If an outer dependency needs an inner yielded dependency during its own cleanup, the inner resource cannot be torn down too early. FastAPI enforces/coordinates valid dependency scope ordering. When mixing `function` and `request` scopes, reason about cleanup dependencies as a context-manager stack.

## 15.6 Exceptions around `yield`

```python
async def get_session():
    session = Session()
    try:
        yield session
    except DatabaseError:
        await session.rollback()
        raise
    finally:
        await session.close()
```

If you catch an exception and intend it to propagate, **re-raise it**. Swallowing exceptions in dependency cleanup can create confusing response behavior.

## 15.7 Transaction boundary pattern

A transaction dependency can own commit/rollback:

```python
async def transaction():
    async with session_factory() as session:
        async with session.begin():
            yield session
```

Whether this is correct depends on service semantics. Some teams prefer explicit transaction management in application services to avoid committing simply because a route returned. The reference recommendation: **make transaction ownership explicit and consistent across the codebase**.

## 15.8 Streaming resource lifetime pattern

Bad:

```text
request-scoped DB session
→ route returns generator backed by that session
→ dependency closes before generator actually reads
```

or the reverse waste:

```text
request-scoped DB session used only for auth
→ 20-minute SSE connection
→ DB session held for 20 minutes unnecessarily
```

Choose scope based on actual consumer lifetime.

## 15.9 Dependency overrides

```python
app.dependency_overrides[get_external_client] = fake_client_dependency
```

Overrides replace the target dependency during resolution. Use for:

- external API fakes;
- test DB sessions;
- auth principals;
- clock/randomness providers.

Always clear/restore overrides:

```python
try:
    ...
finally:
    app.dependency_overrides.clear()
```

or encapsulate in fixtures.

## 15.10 Dependency overrides and sub-dependencies

Overriding a high-level dependency can bypass its entire original sub-dependency graph. That is useful, but a test can accidentally stop exercising important behavior. Maintain both isolated override tests and integration tests with the real dependency graph.

## 15.11 Callable instance dependencies

Parameterized dependency instance:

```python
class Checker:
    def __init__(self, expected: str):
        self.expected = expected

    def __call__(self, q: str = "") -> bool:
        return self.expected in q

contains_bar = Checker("bar")

@app.get("/check")
async def check(ok: Annotated[bool, Depends(contains_bar)]):
    return {"ok": ok}
```

Useful when dependency configuration is application-static but per-route declaration should stay simple.

## 15.12 0.140 dependency-graph performance context

FastAPI 0.140.0–0.140.7 introduced a series of refactors to reduce memory use and repeated dependency flattening, particularly for large applications and OpenAPI generation. 0.141.1 includes those changes.

Agent implications:

- do not copy old private dependency-tree traversal code;
- do not depend on internal “flat dependant” structures;
- benchmark actual current release before compensating for historical dependency overhead;
- use public dependency APIs rather than internals.

## 15.13 Advanced dependency anti-patterns

- Long-lived stream holding an auth-only DB session.
- `scope="function"` on a resource that the stream still needs.
- Catching exceptions after `yield` and not re-raising.
- Test override leaking into another test.
- Overriding high-level dependency and assuming sub-dependencies were tested.
- Direct access to private flattened dependency structures.
- Transaction commit semantics hidden in surprising dependency layers.

## 15.14 Agent checklist

```text
[ ] For each yield dependency, choose request vs function scope intentionally.
[ ] Verify nested cleanup order/resource ownership.
[ ] Re-raise caught exceptions unless intentionally translated.
[ ] Define transaction ownership policy.
[ ] Clean dependency overrides reliably.
[ ] Keep integration tests for real dependency graph.
[ ] Avoid private dependency graph internals, especially pre-0.140 patterns.
```

### Sources

- Depends reference: https://fastapi.tiangolo.com/reference/dependencies/
- Dependencies with yield: https://fastapi.tiangolo.com/tutorial/dependencies/dependencies-with-yield/
- Advanced dependencies: https://fastapi.tiangolo.com/advanced/advanced-dependencies/
- Testing overrides: https://fastapi.tiangolo.com/advanced/testing-dependencies/
- Release notes: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 16) Lifespan, application state, and background tasks

## 16.0 Three different lifetime mechanisms

Do not confuse:

| Mechanism | Lifetime / semantics |
|---|---|
| lifespan | once per application process startup/shutdown |
| dependency | normally per request / dependency graph |
| `BackgroundTasks` | in-process work attached to a response after handling |

## 16.1 Lifespan

```python
from contextlib import asynccontextmanager
from fastapi import FastAPI

@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.http = AsyncClient()
    app.state.model = await load_model()
    try:
        yield
    finally:
        await app.state.http.aclose()
        await app.state.model.close()

app = FastAPI(lifespan=lifespan)
```

Use for shared expensive resources used across many requests.

## 16.2 Why not import-time initialization?

Import-time:

```python
model = load_large_model()
```

runs whenever the module is imported, including:

- unit tests that never start the app;
- CLI/schema tooling;
- pre-fork/import phases;
- migrations/scripts importing shared modules.

Lifespan ties initialization to actual application serving/testing lifecycle.

## 16.3 Application state

```python
app.state.http
```

`state` is untyped. Prefer a dependency/accessor that converts it into typed application context:

```python
@dataclass
class AppResources:
    http: AsyncClient
    catalog: Catalog


def get_resources(request: Request) -> AppResources:
    return request.app.state.resources
```

This localizes unsafe/untyped state access.

## 16.4 Legacy startup/shutdown events

`@app.on_event("startup")` / `shutdown` are legacy/deprecated in current docs in favor of lifespan. Maintain them only where existing integrations require them; migrate new code to one coherent lifespan context.

## 16.5 Sub-application lifespan

Mounted sub-applications have their own ASGI lifespan considerations. Do not assume a parent FastAPI lifespan automatically manages arbitrary child-app resources in every deployment/test arrangement. Test mounted-app startup/shutdown explicitly.

## 16.6 `BackgroundTasks`

```python
from fastapi import BackgroundTasks

@app.post("/notifications")
async def notify(tasks: BackgroundTasks):
    tasks.add_task(send_email, "user@example.com")
    return {"accepted": True}
```

This schedules in-process work associated with the response. It is convenient for small, non-durable work.

## 16.7 What `BackgroundTasks` is not

It is not a durable distributed job queue. Risks:

- process crash loses pending/in-progress task;
- deployment restart can interrupt work;
- heavy CPU work consumes web-process resources;
- no built-in cross-node retry/scheduling/durability semantics comparable to a job system.

Use a real worker/broker/task system for important, long-running, retryable, or horizontally distributed jobs.

## 16.8 Dependencies can contribute background tasks

A `BackgroundTasks` object can flow through dependencies and handler logic. FastAPI combines task additions into the response background task lifecycle. Recent 0.141.1 fixes specifically addressed background tasks and headers contributed by dependencies for `app.frontend()` routes, confirming that frontend routes participate in the dependency/response system.

## 16.9 Background task failure handling

Once the response has been sent, a background task exception cannot simply become a new HTTP error response. Log/trace task exceptions and use durable queues when success must be observable/retried.

## 16.10 Graceful shutdown

Lifespan cleanup should:

- stop accepting new resource work where relevant;
- flush/close clients cleanly;
- not hang indefinitely;
- cooperate with server shutdown timeouts.

Long detached tasks can conflict with graceful shutdown. Keep web-process background work bounded.

## 16.11 Lifespan testing

```python
with TestClient(app) as client:
    ...
```

activates lifespan. For async HTTPX tests, `ASGITransport` does not itself trigger lifespan; use an explicit ASGI lifespan manager when needed.

## 16.12 Anti-pattern inventory

- DB pool per request.
- Huge ML model initialized at module import by default.
- `BackgroundTasks` for payments/data pipelines requiring durability.
- Background CPU-bound task running for minutes in web worker.
- Untyped `app.state.*` access scattered throughout routes.
- No shutdown timeout/cleanup tests.

## 16.13 Agent checklist

```text
[ ] Application-wide resources created/closed in lifespan.
[ ] Request resources live in dependencies.
[ ] App state access wrapped/typed where possible.
[ ] BackgroundTasks only for small non-durable work.
[ ] Durable jobs use external worker system.
[ ] Lifespan executed in tests.
[ ] Shutdown cleanup is bounded and observable.
```

### Sources

- Lifespan: https://fastapi.tiangolo.com/advanced/events/
- Background tasks: https://fastapi.tiangolo.com/tutorial/background-tasks/
- BackgroundTasks reference: https://fastapi.tiangolo.com/reference/background/
- Testing lifespan: https://fastapi.tiangolo.com/advanced/testing-events/

---

# FastAPI Advanced — 17) Security primitives and OpenAPI security schemes

## 17.0 FastAPI security layer

FastAPI's `fastapi.security` helpers combine two roles:

1. parse/validate authentication material from the HTTP request;
2. describe the security scheme in OpenAPI.

They do **not** automatically implement your user database, token issuer, authorization policy, revocation, or identity-provider integration.

## 17.1 `Depends` vs `Security`

Use ordinary `Depends()` for security logic when OAuth2 scopes are not part of the declared contract.

Use:

```python
Security(dependency, scopes=[...])
```

when the dependency should receive/propagate required OAuth2 scopes into OpenAPI and `SecurityScopes`.

## 17.2 API key helpers

FastAPI supports API-key extraction patterns for headers, query params, and cookies through security helper classes. Example concept:

```python
from fastapi.security import APIKeyHeader

api_key = APIKeyHeader(name="X-API-Key")

@app.get("/private")
async def private(key: Annotated[str, Security(api_key)]):
    ...
```

The helper extracts the key; your code must validate it against a secure backend/hash/store.

## 17.3 HTTP Basic

```python
from fastapi.security import HTTPBasic, HTTPBasicCredentials

basic = HTTPBasic()

@app.get("/whoami")
async def whoami(
    credentials: Annotated[HTTPBasicCredentials, Security(basic)],
):
    ...
```

Use HTTPS; Basic credentials are merely encoded for transport, not encrypted independently.

Constant-time comparison is relevant when comparing secrets directly.

## 17.4 HTTP Bearer

```python
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

bearer = HTTPBearer()

async def credentials(
    value: Annotated[HTTPAuthorizationCredentials, Security(bearer)],
):
    token = value.credentials
```

`HTTPBearer` parses the Authorization scheme; your application still verifies token signature/claims/introspection.

## 17.5 OAuth2 password bearer helper

```python
from fastapi.security import OAuth2PasswordBearer

oauth2 = OAuth2PasswordBearer(tokenUrl="token")

async def current_token(token: Annotated[str, Depends(oauth2)]):
    ...
```

This expresses an OAuth2 password-flow-style OpenAPI security scheme and extracts the bearer token. It does not create `/token` or verify JWTs automatically.

## 17.6 OAuth2 authorization-code helper

Current security reference includes `OAuth2AuthorizationCodeBearer(...)` for authorization-code flows, with parameters including authorization URL, token URL, optional refresh URL, scopes, description, and error behavior.

This is appropriate when the API delegates authentication to an OAuth/OIDC authorization server and wants OpenAPI tooling to know the flow.

## 17.7 OpenID Connect helper

FastAPI provides OpenID Connect-related security declaration support for discovery-based schemes. For real OIDC, use a well-maintained OAuth/OIDC library or identity-provider SDK for discovery, signature keys, claims validation, refresh/logout, and edge cases; FastAPI's scheme declaration is not a full OIDC client implementation.

## 17.8 `auto_error`

Many security helpers have `auto_error=True`. With it, missing credentials cause an automatic error. Setting `False` allows optional/multiple authentication mechanisms:

```python
scheme = HTTPBearer(auto_error=False)
```

Then your dependency must handle `None` explicitly.

## 17.9 Authentication vs authorization

```text
authentication = who is the caller?
authorization  = may this caller perform this action on this resource?
```

FastAPI security schemes mostly assist authentication material extraction and OpenAPI description. Authorization remains application policy.

## 17.10 Secret/key storage

Do not place API keys, JWT signing keys, or client secrets in route code or OpenAPI metadata. Load secrets from a managed secret/config system and rotate them.

## 17.11 Error semantics

Use consistent statuses and headers for auth failures. Depending on scheme:

- missing/invalid authentication often maps to 401;
- authenticated but insufficient permission often maps to 403;
- OAuth2/Bearer challenges may require `WWW-Authenticate` metadata.

Do not leak whether sensitive users/resources exist through inconsistent authorization errors unless the API intentionally does so.

## 17.12 Security helper anti-patterns

- Treating token extraction as token verification.
- Storing plaintext API keys in source/database without hashing/secure design.
- Basic auth over plain HTTP.
- Optional `auto_error=False` without handling unauthenticated `None` paths.
- Implementing OAuth/OIDC protocol details from scratch in route functions.
- Conflating OpenAPI security declaration with enforcement.

## 17.13 Agent checklist

```text
[ ] Choose scheme helper matching actual HTTP auth contract.
[ ] Verify credentials separately from extracting them.
[ ] Use Security() when OAuth scopes must propagate.
[ ] Enforce authorization on resource/action, not only token presence.
[ ] Keep secrets out of source/OpenAPI/logs.
[ ] Use HTTPS for credential-bearing traffic.
```

### Sources

- Security tutorial: https://fastapi.tiangolo.com/tutorial/security/
- Security reference: https://fastapi.tiangolo.com/reference/security/
- Dependencies/Security: https://fastapi.tiangolo.com/reference/dependencies/

---

# FastAPI Advanced — 18) OAuth2, JWT, scopes, authentication, and authorization architecture

## 18.0 Architecture first

A production security design should separate:

```text
credential transport
    -> token/session validation
        -> principal construction
            -> coarse scopes/roles
                -> resource-level authorization
                    -> domain operation
```

Do not make the route depend directly on raw JWT claims everywhere.

## 18.1 Canonical current-user dependency

```python
CurrentUser = Annotated[User, Depends(get_current_user)]

@app.get("/me")
async def me(user: CurrentUser) -> UserRead:
    return user
```

`get_current_user` should be the boundary that turns authentication material into a trusted principal representation.

## 18.2 JWT verification requirements

A safe JWT validator normally checks at least:

- signature against correct key/algorithm;
- expiration (`exp`);
- not-before where used;
- issuer (`iss`) where relevant;
- audience (`aud`) where relevant;
- token type/purpose if access vs refresh vs ID tokens can be confused;
- key rotation/JWK selection;
- claims required by your application.

Never decode a JWT without verification and then trust its claims.

## 18.3 Password storage

Store password hashes using a modern password hashing function/library; never encrypt/store plaintext passwords for authentication. The FastAPI tutorial demonstrates password hashing and JWT token creation as an example architecture, but production parameter selection and breach-response policy remain application/security responsibilities.

## 18.4 OAuth2 scopes

Scopes are strings describing permission capabilities. FastAPI integrates them with OpenAPI and dependency propagation.

Example declaration:

```python
from fastapi import Security

@app.get("/users/me")
async def me(
    user: Annotated[User, Security(get_current_user, scopes=["users:read"])],
):
    return user
```

The required scopes become available to nested security logic through `SecurityScopes`.

## 18.5 `SecurityScopes`

```python
from fastapi.security import SecurityScopes

async def get_current_user(
    security_scopes: SecurityScopes,
    token: Annotated[str, Depends(oauth2_scheme)],
):
    required = set(security_scopes.scopes)
    claims = verify_token(token)
    if not required.issubset(set(claims.scopes)):
        raise HTTPException(status_code=403, detail="Insufficient permissions")
    ...
```

This lets requirements accumulate through dependency chains.

## 18.6 Scope design

Good scopes describe stable capabilities:

```text
users:read
users:write
orders:read
orders:approve
```

Avoid encoding every resource identifier into static OAuth scopes unless your identity provider/policy model is explicitly designed for that. Resource ownership and row-level permissions typically need runtime authorization logic.

## 18.7 Role-based vs attribute/resource-based authorization

Roles/scopes are coarse. Resource-level authorization often needs:

```python
async def get_order_for_user(
    order_id: int,
    user: CurrentUser,
    repo: OrderRepoDep,
) -> Order:
    order = await repo.get(order_id)
    if not policy.can_read_order(user, order):
        raise HTTPException(status_code=404)  # or 403 per disclosure policy
    return order
```

This dependency can then inject an already-authorized resource into the route.

## 18.8 Multi-tenant authorization

Never trust a tenant ID supplied in a header/body simply because it validates syntactically. Bind tenant context to the authenticated principal/token/session and enforce it in data-access policy.

Pattern:

```text
validated token -> principal.allowed_tenants
X-Tenant-ID -> requested tenant
policy -> requested tenant must be allowed
repository -> query scoped to tenant
```

## 18.9 Session/cookie vs bearer tokens

FastAPI supports either HTTP pattern. Choose based on client type and threat model:

| Client | Common approach |
|---|---|
| browser web app same-site | secure HttpOnly session cookie + CSRF design |
| SPA/API on separate domain | OAuth/OIDC tokens or BFF/session pattern |
| mobile/native | OAuth/OIDC authorization code + PKCE |
| service-to-service | client credentials/mTLS/signed service identity |

FastAPI does not force one architecture.

## 18.10 Refresh tokens

Treat refresh tokens as higher-value long-lived credentials:

- separate audience/type;
- secure storage;
- rotation/reuse detection where appropriate;
- revocation/session state;
- never accept a refresh token as an access token simply because both are JWT-shaped.

## 18.11 Authorization caching

Per-request dependency caching is useful for current-user resolution. Cross-request caching of authorization decisions requires invalidation semantics and can create stale privilege/security bugs. Keep policy caching explicit.

## 18.12 Security testing matrix

For each protected endpoint test:

```text
no credential
malformed credential
expired credential
wrong issuer/audience
valid credential, insufficient scope
valid scope, unauthorized resource
valid resource access
cross-tenant attempt
revoked/disabled principal where supported
```

## 18.13 Anti-pattern inventory

- `jwt.decode(..., options={"verify_signature": False})` in trusted flow.
- Role claim accepted without issuer/audience/signature validation.
- Tenant ID controlled only by request header.
- Scopes treated as complete resource-level authorization.
- Refresh and access tokens accepted interchangeably.
- Authorization decisions scattered inconsistently across route bodies.
- Sensitive existence leaked through inconsistent error behavior.

## 18.14 Agent checklist

```text
[ ] Centralize raw token/session -> trusted principal conversion.
[ ] Verify JWT signature and semantic claims.
[ ] Use SecurityScopes for declared OAuth scope requirements.
[ ] Separate coarse scopes from resource-level authorization.
[ ] Bind tenant context to authenticated identity.
[ ] Distinguish access/refresh/ID tokens.
[ ] Test negative security matrix.
```

### Sources

- OAuth2/JWT tutorial: https://fastapi.tiangolo.com/tutorial/security/oauth2-jwt/
- OAuth2 scopes: https://fastapi.tiangolo.com/advanced/security/oauth2-scopes/
- Security reference: https://fastapi.tiangolo.com/reference/security/

---
# FastAPI Advanced — 19) Errors, exceptions, validation failures, and exception handlers

## 19.0 Error-path mental model

FastAPI has several distinct failure classes that should not be collapsed into one generic "exception" concept:

```text
request arrives
  ├─ routing failure / Starlette HTTP error
  ├─ dependency or parameter validation failure
  │    └─ RequestValidationError -> default 422 response
  ├─ application explicitly raises HTTPException
  │    └─ default JSON error response
  ├─ application explicitly raises WebSocketException
  │    └─ WebSocket close/error semantics
  └─ unexpected Python exception
       └─ server error handling / logging / middleware policy
```

The documented public mechanism for intentional HTTP failures is `fastapi.HTTPException`. Request validation failures are represented by `fastapi.exceptions.RequestValidationError`. FastAPI installs default exception handlers for these classes and lets applications override them globally with `@app.exception_handler(...)`.

## 19.1 `HTTPException`

Canonical use:

```python
from fastapi import FastAPI, HTTPException

app = FastAPI()

@app.get("/items/{item_id}")
async def read_item(item_id: str):
    item = await lookup_item(item_id)
    if item is None:
        raise HTTPException(
            status_code=404,
            detail="Item not found",
        )
    return item
```

`HTTPException` is an exception, so **raise** it; do not return it.

Useful surface:

```python
HTTPException(
    status_code: int,
    detail: Any = None,
    headers: dict[str, str] | None = None,
)
```

Use `headers=` when protocol semantics require response headers, for example authentication challenges or retry metadata.

## 19.2 Structured `detail`

`detail` does not have to be a string. JSON-compatible data can be used:

```python
raise HTTPException(
    status_code=409,
    detail={
        "code": "ORDER_ALREADY_FINALIZED",
        "order_id": order_id,
    },
)
```

For product APIs, prefer a stable application error envelope rather than unconstrained ad hoc structures per endpoint.

## 19.3 `RequestValidationError`

Invalid request data produces a `RequestValidationError`. The default FastAPI handler emits an HTTP 422 response containing structured error entries.

Override globally when the external API contract requires another envelope:

```python
from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse

app = FastAPI()

@app.exception_handler(RequestValidationError)
async def validation_exception_handler(
    request: Request,
    exc: RequestValidationError,
):
    return JSONResponse(
        status_code=422,
        content={
            "code": "REQUEST_VALIDATION_FAILED",
            "errors": exc.errors(),
        },
    )
```

Treat validation errors as **caller input failures**, not as application/server failures.

## 19.4 Request validation vs Pydantic `ValidationError`

Do not conflate a `RequestValidationError` caused by external request input with a Pydantic validation failure created internally by application code.

Operational rule:

```text
bad caller input -> safe structured 4xx error
invalid application-produced model/state -> likely programmer/system fault
```

Leaking raw internal Pydantic exceptions to clients can reveal implementation details and incorrectly reclassify server bugs as user mistakes.

## 19.5 Override Starlette HTTP exception handling

FastAPI uses Starlette's underlying HTTP routing/error machinery. If you need one handler for framework/router HTTP errors as well as FastAPI-generated errors, register against Starlette's exception type:

```python
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from starlette.exceptions import HTTPException as StarletteHTTPException

app = FastAPI()

@app.exception_handler(StarletteHTTPException)
async def http_exception_handler(
    request: Request,
    exc: StarletteHTTPException,
):
    return JSONResponse(
        status_code=exc.status_code,
        content={"detail": exc.detail},
        headers=exc.headers,
    )
```

This is useful when 404/405 responses must share the same product error envelope.

## 19.6 Reuse FastAPI default handlers

You can wrap or delegate to FastAPI's built-in handlers instead of reimplementing behavior:

```python
from fastapi import Request
from fastapi.exception_handlers import (
    http_exception_handler,
    request_validation_exception_handler,
)
from fastapi.exceptions import RequestValidationError

@app.exception_handler(RequestValidationError)
async def validation_handler(request: Request, exc: RequestValidationError):
    logger.info("request validation failed", extra={"errors": exc.errors()})
    return await request_validation_exception_handler(request, exc)
```

This is preferable when you only need instrumentation around the standard wire contract.

## 19.7 Domain exceptions -> HTTP mapping

Keep transport errors out of core business logic where possible:

```python
class InsufficientInventory(Exception):
    def __init__(self, sku: str):
        self.sku = sku

@app.exception_handler(InsufficientInventory)
async def inventory_handler(request: Request, exc: InsufficientInventory):
    return JSONResponse(
        status_code=409,
        content={
            "code": "INSUFFICIENT_INVENTORY",
            "sku": exc.sku,
        },
    )
```

Architecture:

```text
domain/service layer -> domain exception
FastAPI adapter layer -> HTTP status + API error envelope
```

This keeps domain code reusable outside HTTP.

## 19.8 Error envelope pattern

Example stable contract:

```python
from pydantic import BaseModel

class ErrorDetail(BaseModel):
    code: str
    message: str
    field: str | None = None

class ErrorResponse(BaseModel):
    error: ErrorDetail
    request_id: str | None = None
```

Document error responses through route `responses=` metadata as well as implementing them at runtime.

## 19.9 Security-sensitive error handling

Avoid:

- returning raw stack traces;
- embedding SQL/database exception text;
- exposing token validation internals;
- revealing whether a protected resource exists when policy intentionally hides that distinction;
- echoing secrets from request bodies into logs or error payloads.

Use a request/correlation ID to connect a generic client error to detailed server telemetry.

## 19.10 Middleware vs exception handler

Use an exception handler when logic is specific to a failure type and should convert that failure to a response.

Use middleware when logic spans both successful and failed requests, e.g. request IDs, timing, tracing, access logs, or security headers.

Avoid a giant catch-all middleware that duplicates FastAPI's validation and HTTP exception machinery.

## 19.11 Anti-pattern inventory

- `return HTTPException(...)` instead of `raise`.
- Converting every exception to HTTP 200 with `{success: false}`.
- Catching `Exception` in every route body.
- Returning raw `exc.errors()` plus full request body when secrets may exist.
- Treating internal Pydantic errors as caller 422 errors.
- Duplicating product error-envelope logic across every route.
- Hiding unexpected server errors from logs while returning generic 500.

## 19.12 Agent checklist

```text
[ ] Raise HTTPException for intentional transport-level failures.
[ ] Distinguish RequestValidationError from internal validation bugs.
[ ] Centralize product error envelopes with exception handlers.
[ ] Preserve headers/status when overriding HTTP exception behavior.
[ ] Map domain exceptions at the adapter boundary.
[ ] Never expose stack traces/secrets in production responses.
[ ] Instrument errors with request/correlation IDs.
[ ] Document non-2xx responses in OpenAPI.
```

### Sources

- Handling errors: https://fastapi.tiangolo.com/tutorial/handling-errors/
- Exceptions reference: https://fastapi.tiangolo.com/reference/exceptions/

---

# FastAPI Advanced — 20) Middleware, CORS, trusted hosts, HTTPS redirects, compression, and sessions

## 20.0 Middleware mental model

Middleware wraps the ASGI application around requests and responses:

```text
client
  -> outer middleware
    -> inner middleware
      -> FastAPI routing + DI + endpoint
    <- inner middleware
  <- outer middleware
<- client
```

Use middleware for cross-cutting request/response behavior, not for ordinary endpoint business logic.

## 20.1 Function-style HTTP middleware

```python
import time
from fastapi import FastAPI, Request

app = FastAPI()

@app.middleware("http")
async def add_process_time_header(request: Request, call_next):
    started = time.perf_counter()
    response = await call_next(request)
    elapsed = time.perf_counter() - started
    response.headers["X-Process-Time"] = f"{elapsed:.6f}"
    return response
```

This is convenient for simple HTTP-only wrappers.

## 20.2 ASGI middleware with `add_middleware`

FastAPI inherits Starlette's middleware system:

```python
app.add_middleware(
    SomeMiddleware,
    option=value,
)
```

Prefer native ASGI middleware for reusable infrastructure and when HTTP/WebSocket behavior both matter.

## 20.3 Middleware ordering

Middleware ordering is semantically significant. A middleware added outside another one sees the request earlier and the response later.

Agent rule: explicitly reason about desired ordering for:

- CORS;
- exception/error capture;
- tracing;
- compression;
- authentication/session parsing;
- security headers.

Do not rely on accidental registration order without tests.

## 20.4 CORS model

An **origin** is protocol + host + port. Different ports are different origins.

Use:

```python
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "https://app.example.com",
        "https://admin.example.com",
    ],
    allow_credentials=True,
    allow_methods=["GET", "POST", "PUT", "DELETE"],
    allow_headers=["Authorization", "Content-Type"],
    max_age=600,
)
```

## 20.5 CORS credentials rule

A wildcard origin configuration does not behave like an explicit trusted-origin list for credentialed browser requests. If cookies or Authorization headers are required, explicitly enumerate trusted origins.

Production stance:

```text
allow_origins=["*"] + credentials -> wrong mental model
explicit trusted origins          -> preferred
```

CORS is a **browser policy**, not API authentication. Non-browser callers can ignore CORS entirely.

## 20.6 Preflight vs simple CORS requests

CORS middleware handles:

- preflight `OPTIONS` requests carrying `Origin` and `Access-Control-Request-Method`;
- normal requests with `Origin`, adding response headers when permitted.

Avoid implementing custom `OPTIONS` routes for ordinary CORS handling.

## 20.7 `TrustedHostMiddleware`

Protect host-header assumptions when the application has a known set of valid hosts:

```python
from fastapi.middleware.trustedhost import TrustedHostMiddleware

app.add_middleware(
    TrustedHostMiddleware,
    allowed_hosts=["api.example.com", "*.api.example.com"],
)
```

Use especially when code builds absolute URLs, host-based tenancy, redirects, or security decisions from host metadata.

## 20.8 `HTTPSRedirectMiddleware`

```python
from fastapi.middleware.httpsredirect import HTTPSRedirectMiddleware

app.add_middleware(HTTPSRedirectMiddleware)
```

This redirects HTTP/WS schemes to secure HTTPS/WSS equivalents. In production behind a TLS termination proxy, ensure forwarded-proxy metadata is configured correctly; otherwise the application may incorrectly believe the external request is HTTP and redirect-loop.

## 20.9 `GZipMiddleware`

```python
from fastapi.middleware.gzip import GZipMiddleware

app.add_middleware(
    GZipMiddleware,
    minimum_size=1000,
    compresslevel=5,
)
```

Compression is useful for sufficiently large compressible responses. Do not assume application-level GZip is always preferable to ingress/CDN compression; benchmark the deployed topology.

Streaming responses can interact with compression buffering and latency. Validate time-to-first-byte for SSE and other incremental streams rather than globally enabling compression without tests.

## 20.10 `SessionMiddleware`

Starlette provides signed cookie-backed session middleware:

```python
from starlette.middleware.sessions import SessionMiddleware

app.add_middleware(
    SessionMiddleware,
    secret_key=settings.session_secret,
    https_only=True,
    same_site="lax",
)
```

The session cookie is signed to prevent tampering; it is not a place to store arbitrary secrets just because clients cannot forge the signature. Design session state deliberately.

## 20.11 Custom security-header middleware

```python
@app.middleware("http")
async def security_headers(request: Request, call_next):
    response = await call_next(request)
    response.headers.setdefault("X-Content-Type-Options", "nosniff")
    response.headers.setdefault("Referrer-Policy", "no-referrer")
    return response
```

Prefer an ingress/CDN policy where organization-wide headers are centrally controlled, but app-level middleware is appropriate for API-specific behavior.

## 20.12 Correlation/request ID middleware

```python
from uuid import uuid4

@app.middleware("http")
async def request_id(request: Request, call_next):
    request_id = request.headers.get("X-Request-ID") or str(uuid4())
    request.state.request_id = request_id
    response = await call_next(request)
    response.headers["X-Request-ID"] = request_id
    return response
```

If accepting caller-provided IDs, validate allowed format/length to avoid log injection or unbounded metadata.

## 20.13 ASGI middleware vs dependency

| Concern | Middleware | Dependency |
|---|---:|---:|
| every request | excellent | app-level dependency possible |
| route parameter/context | awkward | excellent |
| contributes OpenAPI auth schema | no | `Security` yes |
| modifies response headers globally | excellent | possible but less direct |
| WebSocket support | ASGI middleware yes | WebSocket dependencies yes |
| per-router policy | not naturally scoped | excellent |

Use dependencies for semantic endpoint policy; middleware for protocol-wide cross-cutting behavior.

## 20.14 Anti-pattern inventory

- Treating CORS as authentication.
- `allow_origins=["*"]` while expecting credentialed browser behavior to be safely constrained.
- Trusting arbitrary `Host`/forwarded headers.
- Redirecting to HTTPS without proxy-header correctness behind TLS termination.
- Compressing SSE without latency testing.
- Reading entire large request bodies in generic middleware for logging.
- Creating database sessions in middleware when route-scoped DI provides clearer lifetime control.
- Swallowing exceptions in middleware and returning ambiguous responses.

## 20.15 Agent checklist

```text
[ ] Choose middleware only for cross-cutting protocol concerns.
[ ] Document middleware ordering.
[ ] Enumerate credentialed CORS origins explicitly.
[ ] Use TrustedHostMiddleware when host trust matters.
[ ] Configure HTTPS redirects with proxy metadata awareness.
[ ] Benchmark compression in the real deployment topology.
[ ] Treat browser sessions/cookies as a security architecture, not a convenience toggle.
[ ] Propagate request IDs through logs/traces/responses.
```

### Sources

- Middleware: https://fastapi.tiangolo.com/tutorial/middleware/
- CORS: https://fastapi.tiangolo.com/tutorial/cors/
- Advanced middleware: https://fastapi.tiangolo.com/advanced/middleware/
- Starlette middleware: https://www.starlette.io/middleware/

---

# FastAPI Advanced — 21) `APIRouter`, `include_router`, bigger applications, and the current route-tree architecture

## 21.0 Router mental model

`APIRouter` is FastAPI's compositional API grouping primitive:

```text
FastAPI app
  ├─ app-level routes
  ├─ users APIRouter
  │    ├─ GET /users
  │    └─ GET /users/{id}
  └─ admin APIRouter
       └─ nested audit APIRouter
```

An `APIRouter` supports the same fundamental path-operation declaration model as the application and adds reusable grouping metadata such as prefix, tags, dependencies, responses, and custom route class.

## 21.1 Canonical package layout

```text
app/
  __init__.py
  main.py
  dependencies.py
  routers/
    __init__.py
    users.py
    items.py
    admin.py
  domain/
  services/
  repositories/
```

`routers/users.py`:

```python
from fastapi import APIRouter

router = APIRouter(
    prefix="/users",
    tags=["users"],
)

@router.get("")
async def list_users():
    ...

@router.get("/{user_id}")
async def get_user(user_id: str):
    ...
```

`main.py`:

```python
from fastapi import FastAPI
from .routers import users

app = FastAPI()
app.include_router(users.router)
```

## 21.2 Router-level metadata

```python
router = APIRouter(
    prefix="/admin",
    tags=["admin"],
    dependencies=[Depends(require_admin)],
    responses={403: {"description": "Forbidden"}},
)
```

Use router-level metadata when every route shares the policy. It reduces duplicated decorators and makes API boundaries explicit.

## 21.3 Include-time metadata

Metadata can also be applied at inclusion:

```python
app.include_router(
    admin.router,
    prefix="/internal",
    tags=["internal-admin"],
    dependencies=[Depends(require_internal_network)],
    responses={404: {"description": "Not found"}},
)
```

Conceptual combination:

```text
router prefix + include prefix -> final path prefix
router dependencies + include dependencies + app dependencies -> composed dependency policy
router tags + include tags -> documented operation tags
```

## 21.4 Nested routers

```python
api = APIRouter(prefix="/api")
v1 = APIRouter(prefix="/v1")
users = APIRouter(prefix="/users")

v1.include_router(users)
api.include_router(v1)
app.include_router(api)
```

Keep nesting semantically meaningful. Excessive prefix layering makes final paths and dependency provenance difficult to audit.

## 21.5 Same router under multiple prefixes

FastAPI supports including the same router multiple times:

```python
app.include_router(router, prefix="/api/v1")
app.include_router(router, prefix="/api/latest")
```

Useful for aliases/migrations, but document operation IDs and client-generation implications carefully.

## 21.6 FastAPI 0.137+ live router architecture

A major current-version architectural point: FastAPI 0.137.0 changed router composition so included `APIRouter` and `APIRoute` instances remain active instead of being recreated/cloned during inclusion.

Practical consequences in current FastAPI:

- custom `APIRouter` and `APIRoute` subclasses continue to participate after inclusion;
- adding routes to an already-included router can be reflected by the parent;
- including a child router after its parent was already included can also be reflected;
- router composition is better modeled as a **live tree** than as a one-time copy operation.

This is materially different from older mental models and should be explicit in agent-authored infrastructure.

## 21.7 Do not mutate `router.routes` directly

The current bigger-applications docs explicitly advise against directly mutating `router.routes` after inclusion. `router.routes` is no longer a safe assumption for "plain flat list of final APIRoute objects" in advanced tooling.

Use documented APIs:

```text
@router.get / post / ...
router.add_api_route(...)
router.include_router(...)
```

For tooling that needs route contexts, prefer current documented introspection utilities rather than custom assumptions about internal list shape.

## 21.8 Inclusion is not ASGI mounting

`include_router()` and `mount()` are semantically different:

```text
include_router:
  - API operations become part of same FastAPI/OpenAPI application
  - metadata/dependencies compose
  - shared application routing model

mount:
  - separate ASGI subapplication at a path
  - separate routing/app lifecycle concerns
  - subapp docs/OpenAPI can be independent
```

Do not use `mount()` merely to organize API modules.

## 21.9 Versioned APIs

Prefer explicit composition:

```python
v1 = APIRouter(prefix="/v1")
v2 = APIRouter(prefix="/v2")

v1.include_router(users_v1.router)
v2.include_router(users_v2.router)

app.include_router(v1, prefix="/api")
app.include_router(v2, prefix="/api")
```

Do not mutate a single router differently per request or tenant. Static API topology should be constructed at startup/import time.

## 21.10 Router dependency boundaries

Good pattern:

```python
public = APIRouter(prefix="/public")
internal = APIRouter(
    prefix="/internal",
    dependencies=[Depends(require_internal_identity)],
)
admin = APIRouter(
    prefix="/admin",
    dependencies=[Security(require_user, scopes=["admin"])],
)
```

This turns routing structure into a visible policy map.

## 21.11 Router architecture anti-patterns

- One 3,000-line `main.py` containing every route.
- Router modules importing the `FastAPI` singleton and registering themselves by side effect.
- Mutating `router.routes` directly.
- Using inclusion order as hidden security policy.
- Nesting prefixes so deeply the final URL cannot be inferred locally.
- Duplicating route metadata on every endpoint instead of grouping shared concerns.
- Treating `include_router` as a separate process/subapplication boundary.

## 21.12 Agent checklist

```text
[ ] One router per coherent API/domain surface.
[ ] Put shared prefix/tags/dependencies/responses at router or include boundary.
[ ] Model 0.137+ router inclusion as a live tree, not cloned routes.
[ ] Never directly mutate router.routes in generated application code.
[ ] Use include_router for one API; mount for actual subapplications.
[ ] Make security boundaries visible in router composition.
[ ] Test final OpenAPI paths/operation IDs after nested or repeated inclusion.
```

### Sources

- Bigger applications: https://fastapi.tiangolo.com/tutorial/bigger-applications/
- APIRouter reference: https://fastapi.tiangolo.com/reference/apirouter/
- Release notes 0.137: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 22) Custom `APIRoute`, request adaptation, and route-level instrumentation

## 22.0 Role

FastAPI exposes `APIRoute` as an advanced extension point for changing how selected API routes process requests. It can be a better fit than global middleware when behavior is route-scoped and needs access to FastAPI's route handler lifecycle.

## 22.1 Custom `APIRoute.get_route_handler()`

Canonical timing example:

```python
import time
from collections.abc import Callable
from fastapi import APIRouter, FastAPI, Request, Response
from fastapi.routing import APIRoute

class TimedRoute(APIRoute):
    def get_route_handler(self) -> Callable:
        original = super().get_route_handler()

        async def custom_handler(request: Request) -> Response:
            started = time.perf_counter()
            response: Response = await original(request)
            response.headers["X-Response-Time"] = str(
                time.perf_counter() - started
            )
            return response

        return custom_handler

router = APIRouter(route_class=TimedRoute)

@router.get("/timed")
async def timed():
    return {"ok": True}

app = FastAPI()
app.include_router(router)
```

## 22.2 Route-scoped request-body transformation

The official recipe demonstrates custom `Request` + `APIRoute` for cases such as gzip-decompressing request bodies before FastAPI parses them.

Mental model:

```text
ASGI request
 -> custom APIRoute wrapper
   -> custom Request behavior
     -> normal FastAPI dependency/body validation
       -> endpoint
```

Use only when the protocol adaptation is real and cannot be expressed with ordinary body types or middleware cleanly.

## 22.3 Route class on `APIRouter`

```python
router = APIRouter(route_class=MyCustomRoute)
```

This is the preferred way to scope custom route behavior to a coherent API subset.

## 22.4 Exception-context route wrapper

A custom route handler can catch `RequestValidationError`, inspect safe request metadata, and add context. Be extremely cautious about logging raw bodies because credentials, personal data, or large binary payloads may be present.

## 22.5 Middleware vs APIRoute decision

| Need | Best default |
|---|---|
| every HTTP request | middleware |
| HTTP + WebSocket ASGI behavior | ASGI middleware |
| one router subset | custom `APIRoute` |
| body adaptation before FastAPI validation | custom `APIRoute`/`Request` |
| OpenAPI/DI-aware endpoint policy | dependency |
| simple business rule | endpoint/service layer |

## 22.6 Current router-tree compatibility

Because current FastAPI preserves original router and route objects through inclusion, custom route subclasses are a stronger composition primitive than in older versions where inclusion recreated routes.

Still treat custom `APIRoute` as an advanced extension surface and test upgrades explicitly.

## 22.7 Anti-pattern inventory

- Using custom routes for ordinary authorization that belongs in dependencies.
- Reading and buffering every request body solely for logging.
- Reimplementing validation/serialization inside `APIRoute`.
- Depending on private FastAPI internals instead of `get_route_handler()`.
- Applying a global custom route class when only one router needs it.

## 22.8 Agent checklist

```text
[ ] Use APIRoute only for route-level protocol/instrumentation behavior.
[ ] Call super().get_route_handler() and wrap the returned handler.
[ ] Scope via APIRouter(route_class=...).
[ ] Preserve FastAPI validation, DI, and response conversion.
[ ] Avoid logging secrets/raw large bodies.
[ ] Add focused tests around custom route behavior after FastAPI upgrades.
```

### Sources

- Custom Request and APIRoute: https://fastapi.tiangolo.com/how-to/custom-request-and-route/
- APIRoute reference: https://fastapi.tiangolo.com/reference/apiroute/

---

# FastAPI Advanced — 23) WebSockets

## 23.0 WebSocket mental model

WebSockets are persistent bidirectional connections handled through Starlette's `WebSocket` primitive, with FastAPI adding dependency injection and parameter extraction around the connection endpoint.

```text
HTTP upgrade request
  -> WebSocket route match
  -> FastAPI dependencies / path-query-cookie validation
  -> websocket.accept()
  -> repeated receive/send operations
  -> disconnect / close
```

## 23.1 Minimal endpoint

```python
from fastapi import FastAPI, WebSocket

app = FastAPI()

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    while True:
        data = await websocket.receive_text()
        await websocket.send_text(f"Message: {data}")
```

## 23.2 Path/query/dependency integration

```python
from typing import Annotated
from fastapi import Depends, Query, WebSocket

async def current_ws_user(websocket: WebSocket):
    token = websocket.cookies.get("session")
    return await authenticate_session(token)

@app.websocket("/ws/{channel}")
async def websocket_endpoint(
    websocket: WebSocket,
    channel: str,
    user: Annotated[User, Depends(current_ws_user)],
    since: int | None = Query(default=None),
):
    await websocket.accept()
    ...
```

FastAPI dependency machinery is available for WebSocket routes; use it to keep auth/policy consistent rather than parsing everything manually in each connection loop.

## 23.3 `WebSocketException`

For WebSocket-specific policy failures, use `WebSocketException` with an appropriate close code rather than `HTTPException` after the WebSocket interaction has entered WebSocket semantics.

```python
from fastapi import WebSocketException, status

if not authorized:
    raise WebSocketException(code=status.WS_1008_POLICY_VIOLATION)
```

## 23.4 `WebSocketDisconnect`

When a client disconnects, receive operations can raise `WebSocketDisconnect`:

```python
from fastapi import WebSocketDisconnect

try:
    while True:
        message = await websocket.receive_text()
        ...
except WebSocketDisconnect:
    manager.disconnect(websocket)
```

Cleanup must tolerate disconnects at any point.

## 23.5 Connection manager

Simple single-process pattern:

```python
class ConnectionManager:
    def __init__(self):
        self.active: set[WebSocket] = set()

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active.add(websocket)

    def disconnect(self, websocket: WebSocket):
        self.active.discard(websocket)

    async def broadcast(self, message: str):
        dead: list[WebSocket] = []
        for ws in self.active:
            try:
                await ws.send_text(message)
            except Exception:
                dead.append(ws)
        for ws in dead:
            self.active.discard(ws)
```

This in-memory topology only coordinates connections inside one process.

## 23.6 Multi-process/distributed WebSockets

If multiple workers/containers serve WebSockets, a process-local set cannot broadcast globally.

Production architecture:

```text
client WebSockets
   -> N FastAPI workers
       -> shared pub/sub / broker
          Redis / NATS / Kafka / PostgreSQL LISTEN-NOTIFY / dedicated gateway
```

Connection ownership remains local to each worker; messages/events are distributed through shared infrastructure.

## 23.7 Backpressure and slow clients

Do not broadcast synchronously to thousands of sockets in one serial loop without backpressure design. Consider:

- per-connection outbound queues;
- maximum queue size;
- disconnect policy for consistently slow clients;
- bounded fan-out concurrency;
- message size limits;
- broker partitioning.

## 23.8 Authentication lifetime

A connection may outlive the credential validity interval. Decide whether authorization is checked:

- only during handshake;
- periodically;
- on each privileged message;
- on server-side revocation events.

Long-lived connection security must be designed explicitly.

## 23.9 Message schemas

FastAPI does not automatically make arbitrary WebSocket message frames into path-operation request models. Validate application messages explicitly:

```python
from pydantic import BaseModel, TypeAdapter

class ClientEvent(BaseModel):
    type: str
    payload: dict

adapter = TypeAdapter(ClientEvent)

raw = await websocket.receive_json()
event = adapter.validate_python(raw)
```

For mature systems, define discriminated unions for event types rather than an unconstrained `dict` payload.

## 23.10 Testing

`TestClient` supports WebSocket testing:

```python
from fastapi.testclient import TestClient

client = TestClient(app)

with client.websocket_connect("/ws") as websocket:
    websocket.send_text("hello")
    assert websocket.receive_text() == "Message: hello"
```

## 23.11 Anti-pattern inventory

- In-memory global connection list assumed to work across workers.
- No `WebSocketDisconnect` cleanup.
- Unbounded per-client queues.
- Trusting handshake auth forever for long-lived privileged connections without policy.
- Raw unvalidated JSON dict messages.
- Blocking database/network calls inside receive loop.
- Broadcasting serially to very large connection counts without concurrency/backpressure design.

## 23.12 Agent checklist

```text
[ ] Authenticate/authorize before accepting privileged WebSocket use.
[ ] Catch WebSocketDisconnect and clean up idempotently.
[ ] Validate message schemas explicitly.
[ ] Design bounded outbound backpressure.
[ ] Use shared broker/pub-sub for multi-worker fan-out.
[ ] Define credential/session lifetime behavior.
[ ] Test disconnect, malformed message, unauthorized message, slow-client paths.
```

### Sources

- WebSockets: https://fastapi.tiangolo.com/advanced/websockets/
- WebSocket reference: https://fastapi.tiangolo.com/reference/websockets/
- Testing WebSockets: https://fastapi.tiangolo.com/advanced/testing-websockets/

---

# FastAPI Advanced — 24) OpenAPI generation, API metadata, Swagger UI, and ReDoc

## 24.0 OpenAPI mental model

FastAPI's API description is generated mechanically from:

```text
FastAPI application metadata
+ path operation metadata
+ parameter/body/response schemas
+ security schemes
+ callbacks/webhooks/responses
= OpenAPI document
```

Default paths:

```text
/openapi.json -> OpenAPI schema
/docs         -> Swagger UI
/redoc        -> ReDoc
```

All three are configurable or disable-able.

## 24.1 Application metadata

```python
app = FastAPI(
    title="Payments API",
    summary="Internal and external payment operations",
    description="""
    API for payment creation, settlement status, and reconciliation.
    """,
    version="2.4.0",
    terms_of_service="https://example.com/terms",
    contact={
        "name": "Payments Platform",
        "url": "https://example.com/support",
        "email": "payments@example.com",
    },
    license_info={
        "name": "Proprietary",
    },
)
```

This metadata is API-product metadata, not FastAPI package versioning.

## 24.2 Tags metadata

```python
openapi_tags = [
    {
        "name": "payments",
        "description": "Create and inspect payments.",
    },
    {
        "name": "admin",
        "description": "Privileged operational APIs.",
    },
]

app = FastAPI(openapi_tags=openapi_tags)
```

Stable tags improve docs navigation and generated client organization.

## 24.3 Disable docs/schema endpoints

```python
app = FastAPI(
    openapi_url=None,
    docs_url=None,
    redoc_url=None,
)
```

Disabling docs is not a substitute for authorization. If the API itself is exposed, obscuring documentation is not a security boundary.

## 24.4 Custom docs paths

```python
app = FastAPI(
    openapi_url="/api/openapi.json",
    docs_url="/api/docs",
    redoc_url="/api/redoc",
)
```

Ensure proxy/root-path configuration makes these externally reachable at the intended URLs.

## 24.5 Swagger UI configuration

`swagger_ui_parameters=` controls supported Swagger UI options:

```python
app = FastAPI(
    swagger_ui_parameters={
        "deepLinking": True,
        "displayRequestDuration": True,
        "filter": True,
    }
)
```

Treat these as UI options, not API semantics.

## 24.6 OAuth2 redirect configuration

FastAPI constructor fields include:

- `swagger_ui_oauth2_redirect_url`;
- `swagger_ui_init_oauth`.

These configure interactive-doc OAuth behavior. Production OAuth correctness still depends on IdP registration, redirect URIs, PKCE/client type, scopes, and token validation.

## 24.7 Servers metadata

```python
app = FastAPI(
    servers=[
        {"url": "https://api.example.com", "description": "Production"},
        {"url": "https://staging-api.example.com", "description": "Staging"},
    ]
)
```

Be careful with proxy `root_path`: FastAPI can also generate server entries related to root-path behavior.

## 24.8 Operation IDs

Generated OpenAPI clients often turn `operationId` into method names. Stabilize them intentionally for public/client-generated APIs.

```python
@app.get("/users/{user_id}", operation_id="users_get")
async def get_user(user_id: str):
    ...
```

Or provide an application-level `generate_unique_id_function`.

Changing operation IDs can be a generated-client breaking change even when the HTTP path is unchanged.

## 24.9 `include_in_schema`

```python
@app.get("/healthz", include_in_schema=False)
async def healthz():
    return {"ok": True}
```

Use for operational/internal routes that genuinely should not appear in the product schema. It does not make the endpoint private.

## 24.10 `deprecated`

```python
@app.get("/v1/legacy", deprecated=True)
async def legacy():
    ...
```

Marking a route deprecated communicates status to OpenAPI-aware clients/UIs; it does not remove or disable the route.

## 24.11 Schema generation and Pydantic models

Pydantic model schemas feed OpenAPI. FastAPI's `separate_input_output_schemas=True` default can generate distinct input/output representations when defaults/requiredness make them semantically different.

Do not manually duplicate JSON Schema when Pydantic can express the contract directly.

## 24.12 OpenAPI as a contract artifact

For serious APIs, treat generated OpenAPI as testable output:

```python
def test_openapi_contract_snapshot():
    schema = app.openapi()
    assert schema["info"]["version"] == "2.4.0"
    assert "/payments" in schema["paths"]
```

Prefer semantic assertions or normalized contract diffing to brittle byte-for-byte snapshots when non-semantic ordering can change.

## 24.13 Anti-pattern inventory

- API `version=` set to FastAPI package version instead of product/API version.
- Random operation IDs across refactors.
- Disabling docs and calling the API "secure".
- Hand-editing generated OpenAPI JSON without controlling `app.openapi()` behavior.
- Missing response/error schemas for public APIs.
- Using route names/tags as unstable implementation details when clients depend on them.

## 24.14 Agent checklist

```text
[ ] Set product title/version/description deliberately.
[ ] Define stable tags and operation IDs for generated-client APIs.
[ ] Decide docs/OpenAPI exposure separately from authentication.
[ ] Model schemas in Pydantic instead of parallel hand-written JSON Schema.
[ ] Test important OpenAPI paths/schemas/security requirements.
[ ] Account for proxies/root_path in docs server URLs.
```

### Sources

- Metadata/docs URLs: https://fastapi.tiangolo.com/tutorial/metadata/
- FastAPI class reference: https://fastapi.tiangolo.com/reference/fastapi/
- Swagger UI configuration: https://fastapi.tiangolo.com/how-to/configure-swagger-ui/

---

# FastAPI Advanced — 25) Advanced OpenAPI: additional responses, callbacks, webhooks, custom schemas, and client generation

## 25.0 Role

Basic FastAPI annotations describe the common request/response contract. OpenAPI also supports richer protocol relationships that should be modeled when they are part of the real API: multiple status responses, callbacks, webhooks, external docs, custom schema extensions, and generated clients.

## 25.1 Additional responses

```python
from fastapi import FastAPI
from pydantic import BaseModel

class Item(BaseModel):
    id: str
    name: str

class Error(BaseModel):
    code: str
    message: str

@app.get(
    "/items/{item_id}",
    response_model=Item,
    responses={
        404: {
            "model": Error,
            "description": "Item not found",
        },
        409: {
            "model": Error,
            "description": "Conflict",
        },
    },
)
async def get_item(item_id: str):
    ...
```

The `responses=` metadata documents alternative responses; your runtime code must still return/raise behavior that matches the schema.

## 25.2 Response content types

Additional response metadata can describe non-JSON content, examples, headers, and media types. Use this for file/image/binary endpoints instead of pretending every endpoint is JSON.

## 25.3 OpenAPI callbacks

Callbacks describe **outbound requests your API will make later to a client-provided URL** as part of an existing operation.

Use when the client supplies a callback URL and your system later calls it.

Conceptual contract:

```text
client POST /jobs {callback_url: ...}
server 202
... later ...
server POST callback_url {...event...}
```

FastAPI can define callback routers and attach them to the initiating path operation for OpenAPI generation.

## 25.4 OpenAPI webhooks

Webhooks are first-class OpenAPI descriptions of requests your application can send based on application events, not necessarily tied to a callback URL expression from one specific request.

FastAPI's application constructor has a `webhooks` router surface. Use it to document outbound webhook payloads with the same Pydantic-based schema machinery as inbound routes.

## 25.5 Custom OpenAPI generation

Override `app.openapi` only when necessary:

```python
from fastapi.openapi.utils import get_openapi


def custom_openapi():
    if app.openapi_schema:
        return app.openapi_schema

    schema = get_openapi(
        title=app.title,
        version=app.version,
        summary=app.summary,
        description=app.description,
        routes=app.routes,
    )
    schema.setdefault("info", {})["x-logo"] = {
        "url": "https://example.com/logo.png"
    }
    app.openapi_schema = schema
    return schema

app.openapi = custom_openapi
```

Cache the generated schema unless you intentionally need dynamic per-request schema behavior.

## 25.6 Vendor extensions

OpenAPI keys prefixed `x-...` can carry ecosystem-specific metadata. Keep extensions narrowly scoped and documented because generated tooling may ignore them.

## 25.7 Client generation

FastAPI's OpenAPI output can drive generated SDKs. The most important stability inputs are:

- operation IDs;
- paths/methods;
- request/response schemas;
- enum values;
- requiredness/nullability;
- status codes;
- security schemes/scopes.

Treat changes in those fields as contract changes even if Python implementation remains source-compatible.

## 25.8 Unique ID generation

For larger APIs, define a deterministic naming policy for operation IDs rather than relying on implementation symbol names that may be refactored.

Example:

```python
from fastapi.routing import APIRoute


def custom_generate_unique_id(route: APIRoute) -> str:
    tag = route.tags[0] if route.tags else "default"
    return f"{tag}_{route.name}"

app = FastAPI(generate_unique_id_function=custom_generate_unique_id)
```

Ensure the result is actually unique across the full API.

## 25.9 Contract compatibility review

Classify OpenAPI changes:

```text
usually breaking:
  remove path/method
  rename required field
  make optional input required
  narrow enum
  change operationId used by SDKs
  change success response type/status

usually additive:
  add optional field
  add new path
  add new enum? (can still break exhaustive clients)
```

Generated client behavior can make theoretically additive changes practically breaking; validate against real client languages.

## 25.10 Anti-pattern inventory

- `responses=` documentation that runtime never emits.
- Manually maintained OpenAPI file drifting from FastAPI routes.
- Custom `app.openapi()` rebuilding schema every call without caching.
- Operation IDs derived from unstable module paths.
- Webhook/callback payloads described only in prose instead of schemas.
- Treating every OpenAPI diff as equally important instead of semantic compatibility analysis.

## 25.11 Agent checklist

```text
[ ] Document all materially supported response statuses/media types.
[ ] Use callbacks for request-linked outbound callbacks.
[ ] Use webhooks for event-driven outbound API contracts.
[ ] Override app.openapi only for real schema needs.
[ ] Cache custom OpenAPI output.
[ ] Stabilize operation IDs before SDK generation.
[ ] Run semantic contract diffs in CI for public APIs.
```

### Sources

- Additional responses: https://fastapi.tiangolo.com/advanced/additional-responses/
- OpenAPI callbacks: https://fastapi.tiangolo.com/advanced/openapi-callbacks/
- OpenAPI webhooks: https://fastapi.tiangolo.com/advanced/openapi-webhooks/
- Extending OpenAPI: https://fastapi.tiangolo.com/how-to/extending-openapi/
- Generate clients: https://fastapi.tiangolo.com/advanced/generate-clients/

---

# FastAPI Advanced — 26) Frontend serving, static files, templates, mounts, subapplications, and WSGI interoperability

## 26.0 Current frontend surface

FastAPI 0.138.0 introduced `app.frontend()` / `router.frontend()` as a first-class way to serve a built frontend/static SPA alongside API routes. FastAPI 0.139.0 added dependency support for frontend routes; 0.141.x refined directory checking and frontend dependency response behavior.

This is current stable functionality in 0.141.1 and deserves a separate mental model from legacy `StaticFiles` mounting.

## 26.1 `app.frontend()`

Canonical pattern:

```python
from fastapi import FastAPI

app = FastAPI()

app.frontend(
    "/",
    directory="dist",
)
```

Current signature conceptually:

```python
frontend(
    path,
    *,
    directory,
    fallback="auto",
    check_dir="auto",
    ...
)
```

The feature serves **built static frontend output**. It is not server-side rendering and does not run a JavaScript build pipeline for you.

## 26.2 API routes have priority

Frontend serving is intentionally lower priority than path operations, so a SPA at `/` can coexist with API endpoints:

```python
@app.get("/api/health")
async def health():
    return {"ok": True}

app.frontend("/", directory="dist")
```

Requests matching API routes are handled by the API first.

## 26.3 Frontend fallback modes

FastAPI's frontend surface supports fallback behavior for client-side routing, including `"auto"`, explicit index/404 behavior, or no fallback depending on configuration.

Important safety behavior: SPA fallback is intended for HTML navigation requests; missing JavaScript/CSS/images should remain 404 rather than returning `index.html`, and non-GET/HEAD methods should not silently fall through to SPA HTML.

## 26.4 `check_dir="auto"`

Current 0.141 behavior uses `check_dir="auto"` to make missing frontend build directories friendlier in development while still failing appropriately in production-like execution.

The current docs tie behavior to `FASTAPI_ENV`:

```text
FASTAPI_ENV=development -> missing directory can warn/defer
production/default run  -> missing build directory treated as configuration error
```

This supports backend development before a frontend build exists without weakening production checks.

## 26.5 Frontend dependencies

Frontend routes can inherit/apply FastAPI dependencies, enabling policies such as identity/session setup or response-header behavior.

```python
app.frontend(
    "/",
    directory="dist",
    dependencies=[Depends(load_browser_session)],
)
```

Current 0.141.1 specifically includes fixes so frontend dependencies' response headers/background tasks are handled correctly.

## 26.6 `StaticFiles`

Legacy/general static mounting remains useful:

```python
from fastapi.staticfiles import StaticFiles

app.mount(
    "/static",
    StaticFiles(directory="static"),
    name="static",
)
```

Use for simple asset directories, uploads made deliberately public, documentation assets, etc.

## 26.7 Jinja2 templates

```python
from fastapi import Request
from fastapi.templating import Jinja2Templates


templates = Jinja2Templates(directory="templates")

@app.get("/page")
async def page(request: Request):
    return templates.TemplateResponse(
        request=request,
        name="page.html",
        context={"title": "Example"},
    )
```

Jinja2 is an optional dependency included by the standard FastAPI installation surface.

## 26.8 `mount()` and subapplications

```python
from fastapi import FastAPI

main = FastAPI()
sub = FastAPI()

@sub.get("/users")
async def users():
    return []

main.mount("/subapi", sub)
```

Mounted subapplications are independent ASGI apps with their own routes/docs/OpenAPI. This differs from `include_router`, which integrates routes into one API.

## 26.9 `root_path` and mounted apps

Mounted/subpath deployments affect ASGI `root_path`. Test generated URLs, docs assets, OAuth redirects, and frontend paths under the actual proxy/mount topology.

## 26.10 WSGI interoperability

Starlette/FastAPI can host WSGI applications through WSGI middleware for migration/interoperability scenarios. Treat this as a compatibility bridge, not an argument to run blocking legacy request handling indiscriminately inside the main async architecture.

## 26.11 Frontend deployment decision table

| Need | Default primitive |
|---|---|
| built SPA + FastAPI API same app | `app.frontend()` |
| static asset directory | `StaticFiles` mount |
| server-rendered HTML | Jinja2 templates |
| independent FastAPI service under path | `mount(subapp)` |
| modular routes in same API | `include_router()` |
| legacy WSGI app bridge | WSGI middleware |

## 26.12 Anti-pattern inventory

- Serving source frontend project instead of build output.
- Returning SPA `index.html` for missing `.js`/`.css` assets.
- Assuming `app.frontend()` performs SSR/building.
- Using `mount()` instead of routers merely for code organization.
- Publicly mounting an upload directory without access-control review.
- Depending on development missing-directory behavior in production.

## 26.13 Agent checklist

```text
[ ] Use app.frontend for current static-SPA integration.
[ ] Preserve API route precedence over frontend fallback.
[ ] Ensure missing assets/non-GET methods do not receive SPA HTML.
[ ] Set/understand FASTAPI_ENV behavior for frontend directory checks.
[ ] Use StaticFiles for generic static mounts.
[ ] Use templates for server-rendered HTML.
[ ] Use mount only for real ASGI subapplications.
```

### Sources

- Frontend: https://fastapi.tiangolo.com/tutorial/frontend/
- Static files: https://fastapi.tiangolo.com/tutorial/static-files/
- Templates: https://fastapi.tiangolo.com/advanced/templates/
- Subapplications: https://fastapi.tiangolo.com/advanced/sub-applications/
- WSGI: https://fastapi.tiangolo.com/advanced/wsgi/
- Release notes: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 27) Settings, environment variables, configuration ownership, and secrets

## 27.0 Configuration mental model

FastAPI does not prescribe one configuration system, but the official ecosystem pattern uses `pydantic-settings`:

```text
environment / .env / secret source
  -> BaseSettings model
    -> validated typed Settings object
      -> application factories / dependencies / clients
```

Keep configuration loading separate from request handling.

## 27.1 `pydantic-settings`

```python
from pydantic_settings import BaseSettings, SettingsConfigDict

class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    app_name: str = "Example API"
    database_url: str
    jwt_issuer: str
    jwt_audience: str
    debug: bool = False
```

`pydantic-settings` is optional rather than a required FastAPI core dependency.

## 27.2 Cache settings construction

```python
from functools import lru_cache
from typing import Annotated
from fastapi import Depends

@lru_cache

def get_settings() -> Settings:
    return Settings()

SettingsDep = Annotated[Settings, Depends(get_settings)]
```

This avoids re-reading/parsing configuration on every request and makes tests overrideable through DI.

## 27.3 App factory pattern

```python
from fastapi import FastAPI


def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or get_settings()
    app = FastAPI(
        title=settings.app_name,
        debug=settings.debug,
    )
    register_routes(app, settings)
    return app

app = create_app()
```

Value case:

- deterministic tests;
- multiple deployment configurations;
- explicit configuration ownership;
- fewer import-time side effects.

## 27.4 Environment variables vs `.env`

Production secrets should normally come from deployment secret/config mechanisms rather than committed `.env` files.

```text
local developer -> .env can be convenient
CI             -> secret store / CI env
Kubernetes     -> Secret / external secret manager
cloud runtime  -> managed secret service / injected env/file
```

Do not commit real credentials.

## 27.5 Secret types

Pydantic secret types can reduce accidental display:

```python
from pydantic import SecretStr

class Settings(BaseSettings):
    database_password: SecretStr
```

This reduces accidental repr/log leakage; it does not magically secure memory or downstream uses.

## 27.6 Nested settings

Organize large configurations:

```python
from pydantic import BaseModel

class DatabaseSettings(BaseModel):
    url: str
    pool_size: int = 20

class Settings(BaseSettings):
    database: DatabaseSettings
```

Use Pydantic Settings' documented nested-env configuration when environment structure must map into nested models.

## 27.7 Validate configuration at startup

Fail fast for required configuration. Do not defer a missing database URL, issuer, encryption key, or required external endpoint until the first production request.

Application lifespan is a good place to validate connectivity after pure configuration parsing.

## 27.8 `FASTAPI_ENV`

Current FastAPI CLI/frontend behavior recognizes conventional `FASTAPI_ENV` values such as `development` and `production`.

Important current behavior:

- `fastapi dev` sets development mode when appropriate;
- `fastapi run` does not silently redefine your entire product configuration; set production environment intent explicitly when application code depends on it;
- current frontend `check_dir="auto"` uses this environment signal.

Do not turn `FASTAPI_ENV` into a giant bespoke settings system. It is one deployment/environment signal among your typed settings.

## 27.9 Configuration precedence

Document precedence explicitly. Example product rule:

```text
constructor/test override
> process environment
> .env developer file
> code default
```

Then enforce it consistently through one Settings model.

## 27.10 Do not store request state in settings

Settings should be process/application configuration, not mutable request/session data.

Bad:

```python
settings.current_user = user
```

Use request state, context variables, dependencies, or explicit function arguments for request-scoped information.

## 27.11 Anti-pattern inventory

- Calling `Settings()` in every endpoint.
- Reading `os.environ[...]` throughout business code.
- Committing secrets to `.env` or source.
- Printing full settings objects containing secrets.
- Using one untyped global dict for all config.
- Treating `FASTAPI_ENV` as the only source of production security behavior.
- Loading clients/pools at import time before configuration can be overridden/tested.

## 27.12 Agent checklist

```text
[ ] Model config with pydantic-settings.
[ ] Construct/cache settings once per app/process unless dynamic config is intentional.
[ ] Inject settings or pass them explicitly to components.
[ ] Fail fast on missing/invalid required config.
[ ] Separate process settings from request/session state.
[ ] Keep secrets out of source/logs.
[ ] Document precedence and FASTAPI_ENV use.
```

### Sources

- Settings: https://fastapi.tiangolo.com/advanced/settings/
- Pydantic Settings: https://docs.pydantic.dev/latest/concepts/pydantic_settings/
- FastAPI CLI: https://fastapi.tiangolo.com/fastapi-cli/
- Frontend: https://fastapi.tiangolo.com/tutorial/frontend/

---
# FastAPI Advanced — 28) Database integration, request-scoped sessions, transactions, and persistence boundaries

## 28.0 Framework boundary

FastAPI does **not** require a particular database, ORM, or persistence model. The official relational-database tutorial uses SQLModel (built on SQLAlchemy + Pydantic), but the architectural pattern generalizes to SQLAlchemy, async database drivers, document stores, key-value stores, graph databases, and external services.

The stable FastAPI pattern is:

```text
application lifespan
  -> create long-lived engine / connection pool / client

request dependency
  -> acquire short-lived session / transaction / connection
  -> yield to dependency graph + endpoint
  -> commit/rollback policy
  -> close/release
```

Do not create a new database engine or network pool per request.

## 28.1 SQLModel request-session pattern

The official tutorial demonstrates one session per request using a `yield` dependency:

```python
from typing import Annotated
from fastapi import Depends
from sqlmodel import Session


def get_session():
    with Session(engine) as session:
        yield session

SessionDep = Annotated[Session, Depends(get_session)]
```

Then:

```python
@app.get("/heroes/{hero_id}")
def read_hero(hero_id: int, session: SessionDep) -> HeroPublic:
    hero = session.get(Hero, hero_id)
    if hero is None:
        raise HTTPException(status_code=404, detail="Hero not found")
    return hero
```

The dependency owns the session lifetime; the endpoint owns use of the session.

## 28.2 Sync vs async database stack

Choose one coherent I/O stack.

### Synchronous driver/session

```python
@app.get("/users/{user_id}")
def read_user(user_id: str, session: SessionDep):
    return repository.get(session, user_id)
```

FastAPI runs ordinary sync path functions/dependencies through its threadpool behavior.

### Asynchronous driver/session

```python
async def get_session():
    async with async_session_factory() as session:
        yield session

@app.get("/users/{user_id}")
async def read_user(user_id: str, session: AsyncSessionDep):
    result = await session.execute(...)
    ...
```

Do not call blocking DB drivers directly inside `async def` without an intentional offload boundary.

## 28.3 Engine/pool creation belongs at application lifetime

Preferred production shape:

```python
from contextlib import asynccontextmanager
from fastapi import FastAPI

@asynccontextmanager
async def lifespan(app: FastAPI):
    engine = create_engine_or_pool(settings.database_url)
    app.state.db_engine = engine
    try:
        yield
    finally:
        await close_engine_or_pool(engine)

app = FastAPI(lifespan=lifespan)
```

If the library's engine is cheap and process-global creation is explicitly documented, module-level construction can also be valid; application lifespan gives tests and shutdown behavior a more explicit ownership boundary.

Current FastAPI guidance prefers `lifespan=` over the older startup/shutdown event style for new code.

## 28.4 Transaction ownership patterns

There are three common policies. Choose deliberately.

### Endpoint/service commits explicitly

```python
@app.post("/orders")
async def create_order(order: OrderCreate, session: SessionDep):
    result = await service.create_order(session, order)
    await session.commit()
    return result
```

Good when transaction boundaries are business-operation-specific.

### Unit-of-work dependency

```python
async def get_uow():
    async with UnitOfWork() as uow:
        try:
            yield uow
            await uow.commit()
        except Exception:
            await uow.rollback()
            raise
```

Good when every successful request using the dependency represents one transaction, but be careful with streaming/background behavior.

### Service-layer transaction manager

```python
async with uow.transaction():
    ...
```

Good for nested business workflows and non-HTTP reuse.

## 28.5 Dependency cleanup scope and transactions

FastAPI 0.141's dependency system supports `scope="function"` and `scope="request"` for `yield` dependencies.

This matters for database resources:

```text
scope="function"
  cleanup after endpoint function returns,
  before response body is fully sent

scope="request"
  cleanup after response cycle completes
```

If a streaming response does **not** need the DB session after the endpoint returns, `scope="function"` can release the connection earlier. If response generation still depends on the resource, use request lifetime or redesign the stream to detach from live transaction state.

## 28.6 Streaming and open transactions

Bad pattern:

```text
open DB transaction
 -> return slow stream
 -> keep connection/transaction open for minutes
```

Better:

```text
short DB query/transaction
 -> materialize bounded state / IDs / cursor strategy
 -> close transaction/session if possible
 -> stream from appropriate source
```

Long-held transactions create lock/version-retention and connection-pool pressure.

## 28.7 Repository/service layering

Recommended boundary:

```text
FastAPI router
  -> dependency-provided principal + UoW/session
  -> application service
      -> repository
          -> database library
```

Endpoints should primarily translate HTTP input/output, not contain large SQL/ORM workflows.

## 28.8 Input, table, and output models

Do not automatically expose ORM/table models as public API contracts.

Example SQLModel split:

```python
class HeroBase(SQLModel):
    name: str
    age: int | None = None

class Hero(HeroBase, table=True):
    id: int | None = Field(default=None, primary_key=True)
    secret_name: str

class HeroCreate(HeroBase):
    secret_name: str

class HeroPublic(HeroBase):
    id: int
```

Value:

- prevents accidental secret-field output;
- decouples API evolution from persistence schema;
- distinguishes create/update/read semantics.

## 28.9 Partial updates

For PATCH-style models, distinguish unset from explicitly null:

```python
update_data = payload.model_dump(exclude_unset=True)
```

Do not overwrite database values with model defaults merely because a field was omitted from the request.

## 28.10 Pagination

Always bound list queries exposed to clients:

```python
@app.get("/heroes")
def list_heroes(
    session: SessionDep,
    offset: int = 0,
    limit: Annotated[int, Query(ge=1, le=100)] = 100,
):
    ...
```

For large/changing datasets, cursor/keyset pagination is often superior to high-offset pagination.

## 28.11 N+1 and serialization

Pydantic response conversion can touch relationship attributes. If ORM relationships lazily load during response serialization, you can accidentally produce N+1 query patterns or async-session errors.

Production rule: load the exact output shape intentionally in repository/query code; do not treat response serialization as a database-fetch stage.

## 28.12 Connection-pool sizing

Worker topology multiplies database demand:

```text
4 Uvicorn workers
x pool_size 20
= up to ~80 pooled connections before overflow policies
```

Size pools **per process** against database capacity and real concurrency. More web workers can make database contention worse if pool topology is ignored.

## 28.13 Migrations

Do not run schema migrations independently in every worker at boot unless the migration mechanism is specifically designed for concurrency.

Prefer:

```text
deployment migration job / release phase
 -> complete schema migration
 -> start/roll application replicas
```

`metadata.create_all()` is useful for simple examples/tests, not a substitute for a production migration strategy.

## 28.14 Testing database dependencies

FastAPI's `app.dependency_overrides` lets tests replace the production session dependency with a transaction-isolated test database/session.

```python
app.dependency_overrides[get_session] = override_get_session
try:
    ... test ...
finally:
    app.dependency_overrides.clear()
```

Keep test isolation explicit; global override state can leak across tests if not reset.

## 28.15 Anti-pattern inventory

- Creating DB engine/pool inside each request.
- Blocking synchronous driver calls directly in async path functions.
- One session shared globally across concurrent requests.
- Exposing persistence models containing private fields as API outputs.
- Unbounded list endpoints.
- Holding transaction open throughout slow streaming responses unnecessarily.
- Pool sizing without multiplying by worker/process count.
- Running migrations from every worker process.
- Lazy ORM relationship access during serialization without query planning.

## 28.16 Agent checklist

```text
[ ] Choose sync or async DB stack coherently.
[ ] Create engine/pool at process/application lifetime.
[ ] Acquire session/UoW through yield dependency.
[ ] Define transaction ownership explicitly.
[ ] Choose dependency cleanup scope deliberately for streaming.
[ ] Separate API input/output models from persistence models when needed.
[ ] Bound pagination.
[ ] Plan relationship loading; avoid N+1 serialization.
[ ] Size pool x worker count against DB capacity.
[ ] Use dedicated migration workflow.
[ ] Override session dependencies in tests.
```

### Sources

- SQL databases: https://fastapi.tiangolo.com/tutorial/sql-databases/
- Dependencies with yield: https://fastapi.tiangolo.com/tutorial/dependencies/dependencies-with-yield/
- Advanced dependencies: https://fastapi.tiangolo.com/advanced/advanced-dependencies/
- Lifespan: https://fastapi.tiangolo.com/advanced/events/
- Testing a database: https://fastapi.tiangolo.com/how-to/testing-database/

---

# FastAPI Advanced — 29) Synchronous testing with `TestClient`, lifespan, dependency overrides, and WebSockets

## 29.0 Testing stack

FastAPI re-exports Starlette's test client:

```python
from fastapi.testclient import TestClient
```

It lets ordinary synchronous pytest tests call the asynchronous ASGI application using HTTPX/Starlette test infrastructure.

## 29.1 Minimal test

```python
from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()

@app.get("/")
async def root():
    return {"message": "hello"}

client = TestClient(app)

def test_root():
    response = client.get("/")
    assert response.status_code == 200
    assert response.json() == {"message": "hello"}
```

Test through the HTTP boundary for routing, validation, DI, serialization, and middleware behavior.

## 29.2 Lifespan-aware `TestClient`

If application startup/shutdown lifespan is required, use the client as a context manager:

```python
def test_with_lifespan():
    with TestClient(app) as client:
        response = client.get("/")
        assert response.status_code == 200
```

Entering/exiting the context drives lifespan startup/shutdown around the test block.

## 29.3 Dependency overrides

FastAPI exposes:

```python
app.dependency_overrides: dict[Callable, Callable]
```

Example:

```python
async def get_current_user():
    ...

async def override_current_user():
    return User(id="test-user", is_admin=True)


def test_admin_endpoint():
    app.dependency_overrides[get_current_user] = override_current_user
    try:
        with TestClient(app) as client:
            response = client.get("/admin")
            assert response.status_code == 200
    finally:
        app.dependency_overrides.clear()
```

FastAPI uses the override in place of the original dependency and its subdependency path for that dependency.

## 29.4 Pytest fixture pattern for overrides

```python
import pytest

@pytest.fixture
def app_with_test_user():
    app.dependency_overrides[get_current_user] = override_current_user
    yield app
    app.dependency_overrides.clear()

@pytest.fixture
def client(app_with_test_user):
    with TestClient(app_with_test_user) as client:
        yield client
```

This centralizes cleanup and prevents override leakage.

## 29.5 App factory for test isolation

For highly mutable app state, prefer a new app instance per test/session scope:

```python
@pytest.fixture
def app():
    return create_app(test_settings)
```

This is often cleaner than resetting routers/middleware/global state on a singleton application.

## 29.6 Validation tests

Test invalid inputs, not only happy paths:

```python
def test_limit_too_large(client):
    response = client.get("/items?limit=10000")
    assert response.status_code == 422
```

Important contract tests:

- missing required path/query/header/body data;
- wrong types;
- strict content-type behavior;
- unknown/extra model fields according to model policy;
- auth failure;
- permission failure.

## 29.7 Response-model tests

Assert that private/internal fields are filtered:

```python
def test_user_response_hides_password_hash(client):
    response = client.get("/users/u1")
    assert "password_hash" not in response.json()
```

This is a security regression test, not just formatting.

## 29.8 Cookie/session tests

`TestClient` maintains cookie state like an HTTP client. You can test login/session flows across calls:

```python
client.post("/login", json={...})
response = client.get("/me")
```

Also assert cookie attributes through headers where security flags matter.

## 29.9 WebSocket tests

```python
with TestClient(app) as client:
    with client.websocket_connect("/ws") as websocket:
        websocket.send_text("ping")
        assert websocket.receive_text() == "pong"
```

Test disconnect cleanup and authorization failure paths as well.

## 29.10 Background tasks

Ordinary `TestClient` behavior lets you assert side effects after the response in many simple background-task scenarios, but production durability semantics remain different from a distributed job queue. Unit tests should not imply that in-process background tasks are crash durable.

## 29.11 OpenAPI tests

```python
def test_openapi_has_security(client):
    schema = client.get("/openapi.json").json()
    operation = schema["paths"]["/admin"]["get"]
    assert operation["security"]
```

This catches accidental loss of security declarations/route metadata.

## 29.12 Avoid over-mocking

Good test pyramid:

```text
unit tests -> pure domain/service functions
integration tests -> repository + DB/service adapters
FastAPI contract tests -> request routing/DI/validation/auth/response
end-to-end -> deployed topology critical paths
```

Do not replace every dependency in every test; that can make endpoint tests prove only that mocks return what they were told.

## 29.13 Anti-pattern inventory

- Using `TestClient(app)` without context when the test requires lifespan.
- Leaving `dependency_overrides` set after a test.
- Only happy-path tests.
- Comparing full unstable OpenAPI JSON snapshots when semantic assertions suffice.
- Mocking the router/endpoint itself instead of testing HTTP contract.
- Using one global mutable DB state across parallel tests.

## 29.14 Agent checklist

```text
[ ] Use TestClient for normal synchronous pytest contract tests.
[ ] Use context-manager TestClient when lifespan matters.
[ ] Override expensive/external dependencies through app.dependency_overrides.
[ ] Reset overrides reliably.
[ ] Test invalid inputs, auth failures, and response filtering.
[ ] Test WebSockets through websocket_connect.
[ ] Keep pure business logic independently unit-testable.
```

### Sources

- Testing: https://fastapi.tiangolo.com/tutorial/testing/
- Testing lifespan: https://fastapi.tiangolo.com/advanced/testing-events/
- Dependency overrides: https://fastapi.tiangolo.com/advanced/testing-dependencies/
- Testing WebSockets: https://fastapi.tiangolo.com/advanced/testing-websockets/

---

# FastAPI Advanced — 30) Asynchronous testing with HTTPX `AsyncClient` and ASGI transport

## 30.0 When async tests are necessary

Use asynchronous tests when the test itself needs to await asynchronous application code, database clients, brokers, or other async libraries in the same event loop.

FastAPI's official pattern uses AnyIO + HTTPX `AsyncClient` + `ASGITransport`.

## 30.1 Minimal async test

```python
import pytest
from httpx import ASGITransport, AsyncClient

@pytest.mark.anyio
async def test_root():
    transport = ASGITransport(app=app)
    async with AsyncClient(
        transport=transport,
        base_url="http://test",
    ) as client:
        response = await client.get("/")

    assert response.status_code == 200
```

## 30.2 Why `TestClient` is not used inside async test

`TestClient` bridges async ASGI into a synchronous caller. Once the test itself is async, use an async-native client instead of nesting synchronous bridging behavior.

## 30.3 Lifespan caveat

Important documented behavior: HTTPX `AsyncClient` + `ASGITransport` does **not** automatically trigger FastAPI lifespan events.

If startup/shutdown state matters, explicitly manage lifespan, e.g. with an ASGI lifespan manager:

```python
from asgi_lifespan import LifespanManager
from httpx import ASGITransport, AsyncClient

@pytest.mark.anyio
async def test_with_lifespan():
    async with LifespanManager(app):
        transport = ASGITransport(app=app)
        async with AsyncClient(
            transport=transport,
            base_url="http://test",
        ) as client:
            response = await client.get("/")
            assert response.status_code == 200
```

Pin/test the lifecycle helper as part of your test dependency stack.

## 30.4 Async database integration tests

```python
@pytest.mark.anyio
async def test_create_user(async_client, db_session):
    response = await async_client.post(
        "/users",
        json={"name": "Alice"},
    )
    assert response.status_code == 201

    user = await repository.get_by_name(db_session, "Alice")
    assert user is not None
```

This is a core reason to choose async tests: HTTP request plus direct async persistence inspection in one test.

## 30.5 Dependency overrides still apply

`app.dependency_overrides` is application behavior independent of whether the caller uses TestClient or AsyncClient.

Set/reset overrides through async-compatible fixtures just as carefully.

## 30.6 Event-loop/session ownership

Async DB pools and sessions can be event-loop-sensitive. Avoid creating long-lived loop-bound clients at module import before the test runtime exists.

App lifespan + test lifespan management gives clearer ownership.

## 30.7 AnyIO backend

FastAPI's docs use `@pytest.mark.anyio`. If your dependency stack is specifically asyncio-only, ensure test backend selection matches the libraries under test.

## 30.8 Anti-pattern inventory

- Using `TestClient` inside `async def` tests.
- Assuming `AsyncClient` triggers application lifespan.
- Creating event-loop-bound pool/client globally before test loop.
- Mixing sync and async DB sessions in one request stack accidentally.
- Async tests that never await anything except the HTTP client; ordinary TestClient may be simpler.

## 30.9 Agent checklist

```text
[ ] Use HTTPX AsyncClient + ASGITransport for async-native tests.
[ ] Mark tests with AnyIO/appropriate async pytest integration.
[ ] Explicitly manage lifespan when app startup/shutdown matters.
[ ] Keep async clients/pools owned by test/app lifetime.
[ ] Reuse dependency override patterns and clean up state.
```

### Sources

- Async tests: https://fastapi.tiangolo.com/advanced/async-tests/
- HTTPX ASGI transport: https://www.python-httpx.org/advanced/transports/#asgi-transport

---

# FastAPI Advanced — 31) FastAPI CLI: `fastapi dev`, `fastapi run`, entrypoints, and environment behavior

## 31.0 CLI role

The FastAPI CLI is the framework-provided development/deployment launcher layered on an ASGI server stack. It can discover FastAPI application objects, run Uvicorn, and use project configuration rather than requiring every developer/deployment to manually remember an import string.

The CLI is installed by the normal `fastapi[standard]` distribution surface.

## 31.1 Development mode

```bash
fastapi dev main.py
```

Common with uv:

```bash
uv run fastapi dev main.py
```

Development mode is intended for local iteration and includes development-oriented behavior such as reloading.

Do not use auto-reload as a production process-management strategy.

## 31.2 Production-oriented run

```bash
fastapi run main.py
```

Example explicit bind:

```bash
fastapi run app/main.py --host 0.0.0.0 --port 8000
```

This is the convenient production-style direct launcher for a FastAPI app.

## 31.3 Application discovery

The CLI can infer common cases such as an object named `app` in `main.py`, but explicit project configuration is more robust for real repositories.

## 31.4 `pyproject.toml` entrypoint

Recommended project configuration:

```toml
[tool.fastapi]
entrypoint = "app.main:app"
```

Then:

```bash
fastapi dev
fastapi run
```

This makes the application location discoverable to the CLI and other FastAPI-aware tools.

## 31.5 CLI `--entrypoint`

For one-off overrides:

```bash
fastapi dev --entrypoint app.main:app
```

Prefer checked-in `pyproject.toml` configuration when the entrypoint is stable.

## 31.6 `FASTAPI_ENV`

Current FastAPI uses `FASTAPI_ENV` as an environment-mode signal in features such as frontend directory checking.

Current behavior to internalize:

- development command establishes development intent when not already configured;
- production deployment should set environment intent explicitly when application behavior depends on it;
- `fastapi run` should not be treated as a magical source of all environment-specific product configuration.

Example:

```bash
FASTAPI_ENV=production fastapi run
```

Keep product configuration in a typed Settings model rather than branching across many raw environment reads.

## 31.7 Workers

For a single-host multiprocess deployment:

```bash
fastapi run main.py --workers 4
```

Each worker is a separate Python process with separate memory, application lifespan, pools, caches, and module globals.

## 31.8 Proxy headers

Behind a trusted TLS termination/reverse proxy:

```bash
fastapi run main.py --proxy-headers
```

Proxy-header trust must be limited to trusted proxy sources in the actual server configuration. Do not blindly trust caller-supplied forwarded headers from the public Internet.

## 31.9 Root path

When a proxy strips a path prefix:

```bash
fastapi run main.py --root-path /api/v1
```

The route definitions inside the app remain relative to the application; `root_path` communicates the external mount/prefix context.

## 31.10 Direct Uvicorn remains valid

Equivalent direct ASGI launcher:

```bash
uvicorn app.main:app --host 0.0.0.0 --port 8000
```

The FastAPI CLI is a convenience/integration surface, not a new application server protocol.

## 31.11 Agent deployment rules

```text
developer laptop -> fastapi dev
simple production service -> fastapi run / uvicorn, supervised by platform
single VM multi-core -> --workers N may be appropriate
Kubernetes/ECS/etc -> usually one process/container + replicas at orchestration layer
```

## 31.12 Anti-pattern inventory

- `fastapi dev` in production.
- Relying on automatic app discovery in a complex package when entrypoint can be explicit.
- Assuming multiple workers share memory.
- Trusting forwarded headers from untrusted clients.
- Hard-coding prod secrets into CLI command history.
- Using CLI mode as the entire configuration architecture.

## 31.13 Agent checklist

```text
[ ] Add [tool.fastapi] entrypoint for stable projects.
[ ] Use fastapi dev only for development.
[ ] Use fastapi run or direct ASGI server for production process.
[ ] Choose workers based on deployment topology.
[ ] Configure proxy headers/root_path only for trusted proxy architecture.
[ ] Set FASTAPI_ENV deliberately if behavior depends on it.
```

### Sources

- FastAPI CLI: https://fastapi.tiangolo.com/fastapi-cli/
- First steps / entrypoint: https://fastapi.tiangolo.com/tutorial/first-steps/
- Run server manually: https://fastapi.tiangolo.com/deployment/manually/
- Server workers: https://fastapi.tiangolo.com/deployment/server-workers/

---

# FastAPI Advanced — 32) ASGI deployment with Uvicorn and server-process ownership

## 32.0 Framework/server split

FastAPI is an ASGI application framework. It does not itself own sockets/event loops/process management without an ASGI server layer.

```text
network socket
 -> ASGI server (commonly Uvicorn)
   -> FastAPI ASGI application
      -> Starlette routing/middleware
        -> FastAPI DI/validation/endpoint
```

The FastAPI CLI runs an ASGI server for you; direct Uvicorn/Hypercorn deployment remains valid.

## 32.1 Import-string deployment

```bash
uvicorn app.main:app
```

Meaning:

```text
app.main = Python module
app      = ASGI application object in that module
```

Explicit bind:

```bash
uvicorn app.main:app --host 0.0.0.0 --port 8000
```

## 32.2 Do not use `--reload` in production

Reloading watches source files and restarts processes for development. Production needs platform/process supervision, deterministic artifacts, graceful rollout, and health checks instead.

## 32.3 Programmatic Uvicorn

Programmatic server startup can be useful for custom process integration:

```python
import uvicorn

if __name__ == "__main__":
    uvicorn.run(
        "app.main:app",
        host="0.0.0.0",
        port=8000,
    )
```

Prefer import strings when using reload/workers because child-process import behavior is then explicit.

## 32.4 ASGI concurrency model

One Uvicorn worker normally contains:

```text
one Python process
  -> one async event loop
      -> many concurrent async connections/tasks
      -> threadpool offload for sync endpoints/dependencies as applicable
```

Concurrency does not mean CPU-bound Python bytecode becomes magically parallel inside one ordinary interpreter process.

## 32.5 Keep event loop unblocked

Bad:

```python
@app.get("/slow")
async def slow():
    time.sleep(10)
```

Better:

```python
@app.get("/slow")
def slow():
    time.sleep(10)
```

or use a true async client/library:

```python
@app.get("/slow")
async def slow():
    await async_library_call()
```

CPU-heavy work should often move to process workers, a task system, or a specialized compute service.

## 32.6 Timeouts and resource ceilings

Production deployment should define limits outside endpoint code as appropriate:

- keep-alive timeout;
- graceful shutdown timeout;
- request/body limits at proxy/gateway;
- concurrency/admission limits;
- worker restart policy;
- application-level downstream timeouts.

FastAPI itself is only one layer of the runtime envelope.

## 32.7 Graceful shutdown

ASGI lifespan shutdown is the application hook to close pools/clients and stop resources. The process manager/orchestrator must allow enough termination grace for this cleanup and in-flight request policy.

## 32.8 Health endpoints

Separate concepts:

```text
liveness  -> process should be restarted if false
readiness -> instance should receive traffic if true
startup   -> slow startup gating in orchestrators that support it
```

Do not make liveness depend on every downstream service; that can turn a database outage into a restart storm.

## 32.9 Anti-pattern inventory

- Calling blocking libraries in `async def` without offload.
- Production auto-reload.
- No downstream timeout.
- Health endpoint doing expensive deep diagnostics on every probe.
- Application shutdown requiring more time than orchestrator gives it.
- Treating Uvicorn worker concurrency as CPU-parallel execution.

## 32.10 Agent checklist

```text
[ ] Keep ASGI server and FastAPI framework responsibilities distinct.
[ ] Use async libraries in async endpoints; sync endpoints for blocking sync stacks.
[ ] Do not use reload in production.
[ ] Configure downstream/request/process timeouts intentionally.
[ ] Implement application lifespan cleanup.
[ ] Add liveness/readiness semantics appropriate to platform.
```

### Sources

- Manual server: https://fastapi.tiangolo.com/deployment/manually/
- Deployment concepts: https://fastapi.tiangolo.com/deployment/concepts/
- Uvicorn deployment: https://www.uvicorn.org/deployment/
- Lifespan: https://fastapi.tiangolo.com/advanced/events/

---

# FastAPI Advanced — 33) Workers, processes, concurrency, threadpools, and scaling topology

## 33.0 Process topology mental model

```text
load balancer / listening parent
   -> worker process 1 -> own FastAPI app + pools + globals
   -> worker process 2 -> own FastAPI app + pools + globals
   -> worker process 3 -> own FastAPI app + pools + globals
```

Workers do **not** share ordinary Python memory.

## 33.1 Multiple workers

FastAPI/Uvicorn support:

```bash
fastapi run main.py --workers 4
```

or:

```bash
uvicorn app.main:app --workers 4
```

Use when one host/container process topology should exploit multiple CPU cores and external orchestration is not already doing replica scaling.

## 33.2 Worker multiplication effects

Every worker can duplicate:

- application memory;
- ML/model objects;
- DB pools;
- HTTP client pools;
- in-memory caches;
- scheduled background loops;
- metrics process state.

Capacity plan with **process multiplication**, not only per-worker values.

## 33.3 Kubernetes/container orchestration

FastAPI's deployment guidance generally favors one application process per container in cluster orchestrators, letting the platform replicate containers and load balance among them.

Why:

- independent health/restart;
- horizontal autoscaling;
- explicit resource requests/limits;
- simpler per-container telemetry;
- no nested process manager requirement.

A single VM/Docker Compose deployment can justify multiple workers inside one container/process group.

## 33.4 Sync endpoint threadpool

FastAPI/Starlette can execute ordinary `def` endpoints and sync dependencies without directly blocking the event loop by running them in threadpool machinery.

This is useful for synchronous I/O libraries, but the threadpool is finite shared capacity.

Bad architecture:

```text
500 requests
 -> each does 60-second blocking call
 -> threadpool saturation
 -> rising latency/timeouts
```

Prefer bounded downstream timeouts and async-native clients when concurrency is high.

## 33.5 CPU-bound work

Thread offload does not eliminate Python GIL constraints for ordinary CPU-bound Python code.

Options:

- multiple Uvicorn worker processes;
- `ProcessPoolExecutor` for bounded local compute;
- external job queue/worker system;
- specialized numerical/native code that releases the GIL;
- dedicated compute service.

Do not hold an HTTP request open for arbitrarily long batch computation if an asynchronous job API is a better product model.

## 33.6 Free-threaded Python

FastAPI 0.136.0 added support for free-threaded Python 3.14t environments. Treat this as runtime compatibility, not permission to assume every dependency is thread-safe or that existing application state suddenly becomes safe for unrestricted parallel mutation.

Thread safety remains a library/application property.

## 33.7 Shared state

Do not use process-local dictionaries as authoritative shared state when multiple workers/replicas exist.

```text
process-local cache -> performance optimization only
Redis/DB/object store -> shared durable/coordinated state
```

If cache correctness depends on cross-worker invalidation, implement an explicit shared invalidation strategy.

## 33.8 Background tasks and workers

`BackgroundTasks` run in the same application process. A process crash/redeploy can lose unfinished work.

For durable jobs:

```text
request -> persist/enqueue job -> return job id
worker system -> process job -> persist result/status
```

## 33.9 Capacity planning heuristic

Think in dimensions:

```text
HTTP concurrency
x average downstream wait
x worker processes
x DB pool per worker
x memory per process
x background load
```

Benchmark with production-like payloads and downstream latencies.

## 33.10 Anti-pattern inventory

- Expecting Python globals to be shared across workers.
- Multiplying workers without accounting for DB connections/memory.
- Using huge worker count to compensate for blocked event loop.
- Treating threadpool as infinite.
- Scheduling the same singleton periodic job in every worker unintentionally.
- Using in-process BackgroundTasks for must-not-lose work.
- Assuming free-threaded runtime makes third-party libraries thread-safe.

## 33.11 Agent checklist

```text
[ ] Choose process count based on deployment topology, not a magic CPU formula.
[ ] Multiply DB/client pools and memory by worker count.
[ ] Keep event loop free of blocking work.
[ ] Treat sync threadpool as bounded resource.
[ ] Move durable/long jobs to a real worker/queue.
[ ] Keep shared state outside process memory when correctness requires it.
[ ] Test free-threaded runtime separately before adopting it.
```

### Sources

- Server workers: https://fastapi.tiangolo.com/deployment/server-workers/
- Containers: https://fastapi.tiangolo.com/deployment/docker/
- Async/concurrency: https://fastapi.tiangolo.com/async/
- Release notes: https://fastapi.tiangolo.com/release-notes/
- Starlette thread pool: https://www.starlette.io/threadpool/

---

# FastAPI Advanced — 34) Reverse proxies, forwarded headers, HTTPS termination, `root_path`, and subpath deployments

## 34.0 Proxy topology

Common production path:

```text
browser/client
  -> CDN / API gateway / ingress / reverse proxy
      - TLS termination
      - public hostname
      - maybe strips /api/v1 prefix
  -> Uvicorn/FastAPI on internal HTTP address
```

The internal ASGI server must know which proxy metadata is trustworthy so generated redirects/URLs/security assumptions represent the external request correctly.

## 34.1 Forwarded headers

Reverse proxies commonly send metadata describing original:

- scheme (`https`);
- host;
- client IP;
- port.

Uvicorn supports proxy-header processing. Only trust forwarded headers from known proxy sources; otherwise an attacker can spoof scheme/host/client identity metadata.

## 34.2 TLS termination

TLS is often terminated at Nginx, Traefik, a cloud load balancer, API gateway, or ingress controller. FastAPI/Uvicorn then receives internal HTTP.

Correct forwarded metadata prevents:

- redirects from using `http://` externally;
- OAuth callbacks with wrong scheme/host;
- generated absolute URLs with internal addresses.

## 34.3 `--proxy-headers`

FastAPI's container docs show the production pattern behind a trusted TLS termination proxy:

```bash
fastapi run app/main.py --proxy-headers --port 80
```

Use the underlying server's trusted-proxy allowlist controls in real deployments; do not expose proxy-header trust broadly when clients can connect directly.

## 34.4 Path-stripping proxy and `root_path`

Suppose public URL is:

```text
https://example.com/api/v1/items
```

but the proxy strips `/api/v1` and forwards:

```text
/items
```

The FastAPI route remains:

```python
@app.get("/items")
async def items():
    ...
```

Communicate external prefix through:

```bash
fastapi run main.py --root-path /api/v1
```

or:

```python
app = FastAPI(root_path="/api/v1")
```

## 34.5 ASGI `root_path`

`root_path` is an ASGI concept communicating the path prefix under which the app is externally mounted. It is not the same as adding a router prefix to every endpoint.

```text
router prefix -> application route semantics
root_path     -> deployment/mount context
```

Do not hard-code deployment prefixes into domain API routes when the same artifact must run at different ingress paths.

## 34.6 OpenAPI servers and `root_path`

FastAPI can automatically add a server entry derived from `root_path` to generated OpenAPI so Swagger UI calls the externally correct prefixed API.

If you provide custom `servers`, FastAPI can insert the root-path server at the front.

Disable that automatic behavior when appropriate:

```python
app = FastAPI(
    root_path="/api/v1",
    root_path_in_servers=False,
)
```

Then manage OpenAPI servers explicitly.

## 34.7 Mounted subapplications

ASGI mounts also communicate mount location to subapplications through `root_path`. This is why a FastAPI subapp mounted at `/subapi` can generate correct docs URLs relative to its mount.

## 34.8 Redirect correctness

Routes with automatic slash redirect and auth flows can generate redirects. Behind proxies, test:

```text
GET https://public.example.com/foo
 -> Location must remain https://public.example.com/...
not http://internal-service:8000/...
```

This is both correctness and, in some auth contexts, security-sensitive.

## 34.9 Host trust

Forwarded host plus `TrustedHostMiddleware` need a coherent topology. Determine which layer validates allowed public hostnames and which forwarded host the app will receive.

## 34.10 Proxy buffering and streaming

For SSE/streaming endpoints, reverse-proxy buffering/timeouts can defeat application streaming even when FastAPI emits incrementally.

Validate:

- response buffering disabled where required;
- idle/read timeouts support stream duration;
- compression behavior;
- keepalive connection behavior;
- CDN support for SSE.

## 34.11 Anti-pattern inventory

- Trusting all `X-Forwarded-*` from public clients.
- Hard-coding ingress path into every route rather than using `root_path`.
- Enabling HTTPS redirect behind proxy without scheme-forwarding correctness.
- Ignoring generated Swagger/OpenAPI server URLs under subpath deployment.
- Testing only direct localhost, never actual proxy path.
- Proxy buffering SSE/JSONL and assuming FastAPI streaming is broken.

## 34.12 Agent checklist

```text
[ ] Diagram public proxy -> internal server path.
[ ] Trust forwarded headers only from known proxies.
[ ] Configure external scheme/host metadata correctly.
[ ] Use root_path for stripped deployment prefixes.
[ ] Decide root_path_in_servers behavior explicitly.
[ ] Test redirects/docs/OAuth under real proxy hostname/path.
[ ] Disable/adjust proxy buffering for streaming endpoints.
```

### Sources

- Behind a proxy: https://fastapi.tiangolo.com/advanced/behind-a-proxy/
- Containers / proxy headers: https://fastapi.tiangolo.com/deployment/docker/
- Subapplications: https://fastapi.tiangolo.com/advanced/sub-applications/
- Trusted hosts: https://fastapi.tiangolo.com/advanced/middleware/
- Uvicorn proxy headers: https://www.uvicorn.org/settings/#http

---

# FastAPI Advanced — 35) Containers, orchestration, health checks, startup, and graceful shutdown

## 35.0 Container mental model

A production FastAPI container is usually:

```text
immutable image
  -> Python runtime + pinned dependencies
  -> application source/package
  -> fastapi run / uvicorn process
  -> one application process per container in orchestrated clusters
```

Do not treat a container as a miniature VM that must run an unnecessary process-manager stack.

## 35.1 Current Dockerfile pattern

Representative current FastAPI guidance:

```dockerfile
FROM python:3.14

WORKDIR /code

COPY ./requirements.txt /code/requirements.txt
RUN pip install --no-cache-dir --upgrade -r /code/requirements.txt

COPY ./app /code/app

CMD ["fastapi", "run", "app/main.py", "--port", "80"]
```

For uv-based projects, construct an equivalent deterministic install using lockfile-aware uv workflows.

## 35.2 Layer dependency install before source copy

Copy dependency manifests/lockfiles before application source so Docker can reuse the dependency installation layer when only source code changes.

Example uv concept:

```dockerfile
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev
COPY app ./app
```

Adapt paths/virtualenv execution according to the exact uv image/runtime strategy you choose.

## 35.3 Do not use the old Gunicorn FastAPI base image by default

FastAPI's current container docs explain that the historical `tiangolo/uvicorn-gunicorn-fastapi` image was useful before Uvicorn had its current worker-management capabilities. It is no longer the default recommended architecture.

Use a normal Python/base image and run FastAPI/Uvicorn directly unless your infrastructure has a specific reason otherwise.

## 35.4 One process per container in a cluster

For Kubernetes or comparable orchestration:

```text
Deployment replicas = scale unit
container process   = usually one Uvicorn/FastAPI process
Service/Ingress     = load balancing
```

This lets the orchestrator own restarting, rolling updates, autoscaling, placement, and health.

## 35.5 Multiple workers in a single container

Reasonable for a single-server deployment without higher-level replica management:

```bash
fastapi run app/main.py --workers 4
```

But then health, memory, DB pool, and graceful termination apply to the worker group rather than independently schedulable replicas.

## 35.6 Non-root user

Production image hardening should normally avoid running application code as root:

```dockerfile
RUN useradd --create-home appuser
USER appuser
```

Ensure file permissions, temporary directories, Unix sockets, and mounted volumes are compatible.

## 35.7 Read-only filesystem

Where practical:

```text
container root FS -> read-only
explicit writable mounts -> /tmp / cache / generated files only
```

Do not assume local container disk is durable state.

## 35.8 Secrets

Inject secrets through orchestration/secret-management mechanisms. Do not bake `.env`, private keys, or production credentials into image layers.

Remember: removing a secret in a later Docker layer does not necessarily remove it from prior image history.

## 35.9 Liveness

Minimal liveness should answer: **is the process/event loop able to serve?**

```python
@app.get("/health/live", include_in_schema=False)
async def live():
    return {"status": "ok"}
```

Do not make liveness fail because one optional downstream dependency is temporarily unavailable; that can amplify an outage via restart storms.

## 35.10 Readiness

Readiness can represent whether the instance should receive traffic:

```text
startup complete?
critical local resources initialized?
required dependency currently available enough to serve?
```

Keep readiness checks bounded and fast. Apply aggressive timeouts to downstream checks.

## 35.11 Startup probes

For slow application startup (large model load, cache warmup), an orchestrator startup probe can prevent premature liveness/readiness failure while still enforcing a maximum startup period.

## 35.12 Graceful shutdown

Termination sequence should conceptually be:

```text
orchestrator removes readiness / stops new traffic
 -> sends termination signal
 -> server stops accepting new work
 -> in-flight requests get grace period
 -> FastAPI lifespan cleanup closes pools/clients
 -> process exits before hard kill deadline
```

Tune platform termination grace to real maximum graceful-shutdown time.

## 35.13 Background jobs and shutdown

In-process background tasks can be interrupted by shutdown. Durable job systems should persist/enqueue work before acknowledging the API request according to product semantics.

## 35.14 Container resource limits

Set and test CPU/memory resource policy. Worker count and memory-heavy objects must fit the container limit with headroom.

```text
memory limit 2 GiB
4 workers x 700 MiB model = impossible
```

Prefer fewer workers/replicas or shared external model service for large per-process state.

## 35.15 Observability in containers

Write application logs to stdout/stderr in structured form for collection. Export metrics/traces to network collectors rather than depending on local files.

## 35.16 Anti-pattern inventory

- Old prebuilt Gunicorn FastAPI image used by inertia rather than need.
- N workers in each of N Kubernetes replicas without capacity reasoning.
- Running as root unnecessarily.
- Baking secrets into image.
- Storing durable state only on container filesystem.
- Liveness check synchronously interrogating every downstream dependency.
- Termination grace shorter than application cleanup.
- No memory multiplication calculation for worker-local models/caches.

## 35.17 Agent checklist

```text
[ ] Build deterministic image from pinned dependency lock.
[ ] Prefer one process/container under Kubernetes-like orchestration.
[ ] Use direct FastAPI/Uvicorn process; avoid obsolete base images by default.
[ ] Run non-root where possible.
[ ] Inject secrets at runtime.
[ ] Separate liveness/readiness/startup semantics.
[ ] Give lifespan cleanup enough shutdown grace.
[ ] Model memory + DB pools per worker/replica.
[ ] Treat container filesystem as ephemeral unless mounted otherwise.
```

### Sources

- FastAPI in containers: https://fastapi.tiangolo.com/deployment/docker/
- Deployment concepts: https://fastapi.tiangolo.com/deployment/concepts/
- Server workers: https://fastapi.tiangolo.com/deployment/server-workers/
- Lifespan: https://fastapi.tiangolo.com/advanced/events/

---
# FastAPI Advanced — 36) Security hardening and governance

## 36.0 Security model

FastAPI gives you protocol primitives—validation, security scheme declarations, dependency injection, middleware integration, strict content-type behavior—but it does not automatically create a secure product policy.

Production security is layered:

```text
Internet / client
  -> CDN / WAF / rate limiting / DDoS controls
  -> TLS termination / trusted proxy
  -> host + CORS policy
  -> FastAPI authentication dependency
  -> authorization / tenant / resource policy
  -> input validation
  -> service/repository invariants
  -> least-privilege downstream credentials
  -> audit/observability
```

## 36.1 Keep `strict_content_type=True`

Current FastAPI defaults to strict JSON `Content-Type` checking through the `FastAPI(..., strict_content_type=True)` constructor parameter.

Keep this enabled unless a compatibility requirement is understood and documented. The current docs explicitly tie strict content-type checking to protection against a class of CSRF issues involving local/internal unauthenticated services accepting JSON-like bodies under permissive content types.

Disabling boundary validation to accommodate a broken client moves ambiguity into every downstream layer.

## 36.2 Authentication is not authorization

Authentication produces a trusted identity/principal:

```text
credential -> validated principal
```

Authorization answers whether that principal can perform this action on this resource:

```text
principal + action + resource + tenant/context -> allow / deny
```

OAuth scopes are useful coarse-grained declarations but do not replace resource ownership, tenant isolation, entitlements, or application policy.

## 36.3 JWT validation checklist

For JWT bearer tokens, validate the properties relevant to your trust model:

```text
[ ] cryptographic signature / algorithm policy
[ ] issuer
[ ] audience
[ ] expiration
[ ] not-before if used
[ ] token type / purpose
[ ] subject/principal mapping
[ ] required scopes/claims
[ ] revocation/session policy where required
```

Never accept an ID token, refresh token, or arbitrary JWT as an access token merely because its signature is valid.

## 36.4 Cookie-session security

For browser sessions, review:

- `HttpOnly`;
- `Secure`;
- `SameSite`;
- session fixation/rotation;
- expiration/inactivity;
- CSRF architecture for unsafe methods;
- logout/revocation;
- key rotation.

CORS does not prevent CSRF by itself.

## 36.5 CORS allowlists

With credentials, explicitly enumerate allowed origins. Treat origins as exact scheme + host + port boundaries.

Do not derive CORS allowlists directly from untrusted request headers.

## 36.6 Trusted host and proxy metadata

Validate public hosts where host metadata matters. Trust forwarded client/scheme/host headers only from known reverse proxies.

Security-sensitive uses include:

- absolute redirect URLs;
- OAuth callback URLs;
- tenant selection by host;
- audit source IP;
- secure-cookie/scheme behavior.

## 36.7 Request size limits

Pydantic validation happens after the request data reaches application layers; it is not a complete byte-size defense.

Enforce size limits at suitable layers:

```text
proxy/gateway -> total request/body limits
multipart parser/app -> file/form policy
endpoint/service -> semantic object/array count limits
```

For uploads, stream/process rather than loading arbitrary files fully into memory.

## 36.8 File upload security

Do not trust:

- client filename;
- client MIME type;
- extension;
- archive contents;
- embedded metadata.

Production flow:

```text
UploadFile
 -> generated server-side identifier
 -> size/type/content policy
 -> malware/content scanning if required
 -> isolated object storage
 -> metadata record
```

Prevent path traversal by never concatenating raw client filenames into filesystem paths.

## 36.9 Path and URL input

If a tool/end point fetches caller-provided URLs, design SSRF defenses:

- allowed schemes;
- allow/deny host policy;
- DNS/IP range policy;
- redirect validation;
- timeout and size limits;
- credential/header isolation.

Pydantic's `HttpUrl` validates syntax, not whether accessing the URL is safe for your network.

## 36.10 SQL injection and persistence

Use database parameterization/ORM expression APIs. Do not construct SQL by concatenating request strings.

Pydantic type validation does not make raw SQL concatenation safe.

## 36.11 Command/template injection

Do not pass validated strings directly into:

- shell commands with `shell=True`;
- template engines with unsafe source generation;
- dynamic Python execution;
- unsafe deserializers.

Validation should model allowed semantics, not merely check that the input is a string.

## 36.12 Rate limiting and abuse controls

FastAPI core does not impose a universal application rate limiter. Implement rate limiting at an ingress/gateway or a carefully designed application layer with shared state when limits must apply across workers/replicas.

Rate-limit dimensions can include:

- IP/network;
- authenticated principal;
- tenant;
- API key;
- expensive resource/action;
- concurrency.

Do not use worker-local counters when global enforcement is required.

## 36.13 Downstream timeouts

Every external call should have explicit timeout policy. An API whose endpoint timeout is 30 seconds but whose downstream HTTP client can wait indefinitely is not actually bounded.

Use distinct connect/read/write/pool timeouts where the client library supports them.

## 36.14 Secrets and logs

Never log:

- Authorization header;
- session cookie;
- password;
- full access/refresh token;
- private key;
- DB connection URL with password;
- sensitive request body by default.

Implement structured redaction at the logging boundary rather than relying on every caller to remember.

## 36.15 Error details

Return stable client-safe errors and keep detailed exception context in server telemetry.

```text
client: {code, safe message, request_id}
server: stack trace + request_id + sanitized context
```

## 36.16 Docs/OpenAPI exposure

Interactive docs are valuable operationally. Decide access intentionally:

- public API: public docs may be expected;
- internal API: docs may be protected at gateway/network/auth layer;
- high-security environment: docs endpoints may be disabled.

But disabling `/docs` and `/openapi.json` is **not** authentication.

## 36.17 Security headers

For browser-facing applications/frontends, consider policy headers appropriate to the product:

- HSTS at TLS edge;
- Content-Security-Policy;
- `X-Content-Type-Options: nosniff`;
- Referrer-Policy;
- frame embedding policy.

API-only services may need fewer browser-content controls but still benefit from correct MIME and TLS policy.

## 36.18 Dependency/library security

Pin and update FastAPI, Starlette, Pydantic, multipart parsers, Uvicorn, JWT/crypto libraries, and transitive dependencies through a repeatable lock/update process.

Security fixes often land below FastAPI itself, so "FastAPI pinned" does not mean the full ASGI stack is pinned.

## 36.19 Multi-tenancy

Never accept tenant context as authoritative solely from a path/header/body value.

```text
authenticated principal
 -> allowed tenant set
request tenant
 -> must intersect/validate
DB query
 -> always constrained by trusted tenant context
```

Add cross-tenant negative tests.

## 36.20 Security regression matrix

```text
HTTP boundary:
  invalid content type
  oversized body
  malformed multipart
  path traversal filename

Auth:
  missing credential
  malformed credential
  expired credential
  wrong issuer/audience
  insufficient scope

Authorization:
  wrong resource owner
  wrong tenant
  disabled/revoked principal

Proxy/browser:
  spoofed Host
  spoofed forwarded headers
  disallowed CORS origin
  CSRF attempt where cookies are used
```

## 36.21 Anti-pattern inventory

- Disabling strict content type globally for convenience.
- CORS used as API authentication.
- Trusting arbitrary forwarded headers.
- JWT signature verified but issuer/audience/purpose ignored.
- Path/header tenant accepted without identity binding.
- Raw client filename used as storage path.
- Caller-provided URL fetched without SSRF policy.
- Worker-local rate limiting treated as cluster-global.
- Secrets included in structured logs.
- Docs disabled while API itself is unauthenticated.

## 36.22 Agent checklist

```text
[ ] Keep strict_content_type enabled by default.
[ ] Separate authentication from resource-level authorization.
[ ] Validate complete token trust semantics.
[ ] Design cookie CSRF/session behavior explicitly.
[ ] Enumerate credentialed CORS origins.
[ ] Trust proxy metadata only from known proxies.
[ ] Enforce request/file size and content policy.
[ ] Prevent SSRF/path traversal/injection at semantic boundaries.
[ ] Add global rate limits outside worker-local memory when needed.
[ ] Redact secrets in logs/errors.
[ ] Test tenant/resource authorization failures.
```

### Sources

- FastAPI constructor: https://fastapi.tiangolo.com/reference/fastapi/
- Security: https://fastapi.tiangolo.com/tutorial/security/
- OAuth2 scopes: https://fastapi.tiangolo.com/advanced/security/oauth2-scopes/
- CORS: https://fastapi.tiangolo.com/tutorial/cors/
- Behind a proxy: https://fastapi.tiangolo.com/advanced/behind-a-proxy/
- Request files: https://fastapi.tiangolo.com/tutorial/request-files/

---

# FastAPI Advanced — 37) Performance: dependency graphs, validation, serialization, streaming, and workload tuning

## 37.0 Performance mental model

FastAPI request latency is not one number produced by "FastAPI speed". A realistic path is:

```text
server accept / network
 -> middleware
 -> route match
 -> dependency resolution
 -> request parsing + Pydantic validation
 -> application/service logic
 -> database/network/compute
 -> response validation/serialization
 -> server/network flush
```

Optimize the dominant stage measured in production-like profiles.

## 37.1 Current 0.140 dependency-graph improvements

FastAPI 0.140.x contained a concentrated series of memory/performance refactors around dependency trees and OpenAPI generation:

- 0.140.0 reduced dependency memory usage;
- 0.140.1 adjusted dependency LRU cache limits for large applications;
- 0.140.2 stopped retaining flat dependency trees;
- 0.140.3 avoided repeated dependency flattening during OpenAPI;
- 0.140.4 skipped unused repeat bookkeeping;
- 0.140.5 avoided flattening dependencies for body fields;
- 0.140.6 avoided flattening dependencies for request parameters;
- 0.140.7 avoided flattening dependencies for OpenAPI.

Implication: **do not carry forward old large-app workarounds based on pre-0.140 dependency internals without re-benchmarking 0.141.1**.

## 37.2 Dependency caching

`Depends(..., use_cache=True)` is the default. If a dependency appears multiple times in one request dependency graph, FastAPI can reuse the resolved value.

Use `use_cache=False` only when repeated execution is semantically required:

```python
fresh: Annotated[Value, Depends(get_value, use_cache=False)]
```

Disabling caching by default creates redundant DB/auth/client work.

## 37.3 Dependency granularity

Good:

```text
parse credential -> principal
principal -> tenant context
request -> db session
service -> endpoint
```

Avoid dependency graphs with hundreds of tiny wrappers that add no semantic boundary. They are harder to inspect and test even if current FastAPI handles graphs efficiently.

## 37.4 Pydantic validation

Pydantic v2 is highly optimized, but schema complexity and payload size still matter.

Performance rules:

- use precise models instead of deeply unconstrained recursive structures;
- bound collection lengths when product semantics permit;
- avoid parsing the same payload multiple times;
- use discriminated unions for large unions where appropriate;
- do not manually re-validate already trusted typed models without reason.

## 37.5 Response-model validation cost

`response_model` provides correctness/security and schema value; it also performs output validation/filtering/serialization work.

Do not remove it blindly to shave microseconds if it protects a public contract or filters sensitive fields.

If an endpoint is demonstrably high-volume and returns already-safe pre-serialized data, returning a `Response` directly can bypass some conversion—but then **you own the wire contract**.

## 37.6 Direct `Response` escape hatch

```python
from fastapi.responses import Response

@app.get("/preencoded")
async def preencoded() -> Response:
    body = produce_trusted_json_bytes()
    return Response(content=body, media_type="application/json")
```

Use only with trusted serializer/output because FastAPI will not apply the normal Pydantic output filtering to the `Response` body.

## 37.7 `jsonable_encoder`

`jsonable_encoder` converts Python/Pydantic values into JSON-compatible structures for cases where you need the converted data before constructing a response or storing it elsewhere.

Do not call it redundantly if FastAPI's normal response pipeline already handles the object.

## 37.8 JSON response implementations

FastAPI/Starlette provide normal JSON responses, and optional faster serializers such as ORJSONResponse exist when the dependency is installed.

Benchmark realistic payloads. Serialization is often not the bottleneck compared with DB/network time, and swapping serializers cannot compensate for N+1 queries.

## 37.9 `StreamingResponse`

Use streaming when total result size or latency-to-first-byte matters:

```python
from fastapi.responses import StreamingResponse

async def chunks():
    for chunk in source:
        yield chunk
        await asyncio.sleep(0)  # cancellation/cooperative point where appropriate

return StreamingResponse(chunks(), media_type="application/octet-stream")
```

Streaming reduces full-response buffering but introduces connection-lifetime, proxy, cancellation, and backpressure concerns.

## 37.10 First-class JSON Lines

Current FastAPI supports typed JSON Lines streaming with generator/yield return shapes. Prefer it over ad hoc `StreamingResponse` JSON framing when the API semantic is an incremental sequence of structured records.

Benefits:

- item schema validation;
- OpenAPI representation;
- serialization integration;
- lower memory than building a giant JSON array.

## 37.11 SSE

Current FastAPI supports first-class SSE. Use it for one-way server-to-browser/client event streams, not for bidirectional messaging.

Performance constraints:

- each connection is long-lived;
- worker/concurrency counts must include idle/open streams;
- proxies must not buffer;
- per-client queues must be bounded;
- heartbeats can be required by infrastructure.

## 37.12 File handling

`UploadFile` is generally preferable to `bytes` for large uploads because the underlying uploaded file can be spooled rather than necessarily keeping the entire file as one bytes object in application memory.

Stream file processing where possible.

## 37.13 Sync vs async endpoints

Use `async def` when you can `await` async I/O.

Use `def` for blocking synchronous libraries rather than blocking the main event loop from an `async def` endpoint.

Do not mechanically convert CPU-bound functions to `async def`; that does not make them non-blocking.

## 37.14 Connection pools

HTTP and DB clients should be long-lived/pool-reusing at process/application scope. Creating a fresh client per request often loses connection reuse and adds TCP/TLS setup overhead.

Correct shape:

```text
lifespan -> AsyncClient / DB engine
request -> lightweight use/session
shutdown -> close pool/client
```

## 37.15 OpenAPI generation

Large apps with many routes/models/dependencies can make OpenAPI construction nontrivial. Cache `app.openapi_schema` rather than regenerating custom schema on every request. Current 0.140 optimizations materially reduce dependency-related OpenAPI work.

## 37.16 Router inclusion performance

FastAPI's docs state router inclusion is designed to be lightweight and does not add per-request overhead merely because routes are organized into multiple modules/routers.

Do not collapse a well-structured application into one file for imagined routing speed.

## 37.17 Worker count

More workers can increase throughput until another resource bottleneck dominates. It can also:

- multiply memory;
- multiply DB connections;
- duplicate cache/model loads;
- increase contention.

Benchmark worker counts on the deployed CPU/memory/downstream stack.

## 37.18 Profiling priorities

Measure separately:

```text
p50 / p95 / p99 latency
requests/sec
error/timeouts
CPU per worker
RSS memory per worker
threadpool saturation
DB pool wait
external HTTP pool wait
serialization time
OpenAPI startup/schema time
stream active connection count
```

Use tracing/profiling to find dominant spans before changing framework code.

## 37.19 Anti-pattern inventory

- Optimizing router module count.
- `use_cache=False` everywhere.
- Removing response models without measuring and reviewing security impact.
- Creating DB/HTTP clients per request.
- Blocking sync I/O inside `async def`.
- `list(...)` / `bytes(...)` fully materializing large streams before response.
- Adding many workers while DB is already saturated.
- Assuming ORJSON fixes database/network bottlenecks.
- Carrying old pre-0.140 dependency memory workarounds forward untested.

## 37.20 Agent checklist

```text
[ ] Profile the actual request stages before optimizing.
[ ] Keep dependency caching on unless semantics require fresh values.
[ ] Re-benchmark large apps on 0.141.1 due to 0.140 dependency refactors.
[ ] Preserve response models where contract/security value matters.
[ ] Reuse long-lived connection pools.
[ ] Use first-class JSONL/SSE for matching streaming semantics.
[ ] Use UploadFile/streaming for large files.
[ ] Choose sync vs async by underlying I/O library.
[ ] Benchmark worker count with DB/memory multiplication included.
```

### Sources

- Release notes 0.140.x: https://fastapi.tiangolo.com/release-notes/
- Sub-dependencies/cache: https://fastapi.tiangolo.com/tutorial/dependencies/sub-dependencies/
- Custom responses: https://fastapi.tiangolo.com/advanced/custom-response/
- JSON Lines: https://fastapi.tiangolo.com/tutorial/stream-json-lines/
- SSE: https://fastapi.tiangolo.com/tutorial/server-sent-events/
- Files: https://fastapi.tiangolo.com/tutorial/request-files/
- Async/concurrency: https://fastapi.tiangolo.com/async/

---

# FastAPI Advanced — 38) Large-application architecture and package boundaries

## 38.0 Goal

FastAPI makes it easy to create a whole application in one module. Production maintainability depends on preserving clear boundaries as the codebase grows.

A scalable default:

```text
app/
  main.py                # app factory / composition root
  settings.py            # typed config
  lifespan.py            # shared client/pool startup
  api/
    dependencies.py      # HTTP-facing shared DI
    errors.py            # domain -> HTTP handlers
    routers/
      users.py
      orders.py
      admin.py
  domain/
    users/
      models.py
      services.py
      policies.py
    orders/
      ...
  infrastructure/
    db/
      engine.py
      repositories.py
    http/
      clients.py
    messaging/
  schemas/               # shared API schemas if not domain-local
  observability/
  tests/
```

## 38.1 Composition root

Keep `main.py` small:

```python
from fastapi import FastAPI


def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or get_settings()
    app = FastAPI(
        title=settings.app_name,
        lifespan=create_lifespan(settings),
    )
    install_middleware(app, settings)
    install_exception_handlers(app)
    install_routes(app)
    return app

app = create_app()
```

The composition root wires objects; it should not contain core business workflows.

## 38.2 Router ownership

One router per coherent API capability, not necessarily one router per database table.

Good boundaries:

- `/users` identity profile operations;
- `/orders` commerce workflow;
- `/admin` privileged operations;
- `/events` streaming surface.

Weak boundary:

- router per CRUD table when product semantics span several tables.

## 38.3 Dependency ownership

Classify dependencies:

```text
protocol dependencies:
  headers/cookies/request/client IP

auth dependencies:
  raw credential -> principal -> permissions

resource dependencies:
  DB session, unit of work

application adapters:
  service/client access
```

Do not turn every business function into a FastAPI dependency; business services should remain callable outside request context.

## 38.4 Annotated dependency aliases

Useful for repeated public signatures:

```python
from typing import Annotated

CurrentUser = Annotated[User, Depends(get_current_user)]
SessionDep = Annotated[Session, Depends(get_session)]
SettingsDep = Annotated[Settings, Depends(get_settings)]
```

This keeps path-operation signatures legible while retaining type information.

## 38.5 Service layer

```python
@app.post("/orders", response_model=OrderPublic)
async def create_order(
    payload: OrderCreate,
    user: CurrentUser,
    uow: UowDep,
):
    return await order_service.create(
        uow=uow,
        actor=user,
        command=payload,
    )
```

The service should enforce business invariants independent of HTTP response mechanics.

## 38.6 Repository/client protocols

Use Python protocols/interfaces to make dependencies replaceable/testable without making FastAPI's DI container responsible for every object relationship.

```python
class OrderRepository(Protocol):
    async def get(self, order_id: str) -> Order | None: ...
    async def save(self, order: Order) -> None: ...
```

## 38.7 App state vs globals

Long-lived runtime clients can live in lifespan-owned state:

```python
app.state.http_client = client
```

But raw `app.state` access everywhere creates ambient coupling. Hide it behind one dependency:

```python
def get_http_client(request: Request) -> AsyncClient:
    return request.app.state.http_client
```

## 38.8 Router inclusion policy

Centralize final topology:

```python
def install_routes(app: FastAPI) -> None:
    app.include_router(users.router, prefix="/api/v1")
    app.include_router(orders.router, prefix="/api/v1")
    app.include_router(admin.router, prefix="/api/v1/admin")
```

Avoid modules that import the singleton app and mutate it by import side effect.

## 38.9 Current live route tree

Because 0.137+ preserves routers/routes through inclusion, dynamic additions after inclusion can be visible. This enables powerful composition but can also make accidental late mutation harder to reason about.

Production posture:

- construct topology deterministically during startup/import;
- do not mutate routes based on requests;
- do not treat `router.routes` as public flat internals.

## 38.10 API schema placement

Options:

```text
feature-local schemas -> easiest ownership for modular domains
shared schemas package -> useful for truly cross-feature contracts
persistence models    -> keep separate when API/persistence lifecycles differ
```

Avoid one giant `schemas.py` with hundreds of unrelated models.

## 38.11 Versioning strategy

Do not create `/v2` merely because one field changed. Version at a coherent compatibility boundary.

Possible architectures:

- path versioning (`/api/v1`);
- host versioning;
- media/header versioning with advanced custom routing;
- additive evolution under one stable version.

Current 0.137 router customization enables more sophisticated routing experimentation, but undocumented alpha router methods should not become a production contract without explicit version-risk acceptance.

## 38.12 Plugin/modular registration

If application capabilities are discovered dynamically, define an explicit plugin protocol:

```python
class ApiModule(Protocol):
    def register(self, app: FastAPI) -> None: ...
```

Prefer deterministic startup registration over arbitrary import-time discovery magic.

## 38.13 Circular-import avoidance

Direction:

```text
api -> application/domain -> infrastructure abstractions
composition root -> concrete infrastructure + api routers
```

Avoid repositories importing routers or domain models importing the FastAPI app.

## 38.14 Test architecture

Mirror boundaries:

```text
tests/unit/domain/
tests/unit/services/
tests/integration/db/
tests/api/
tests/e2e/
```

This keeps FastAPI request tests focused on transport contract rather than substituting for all business tests.

## 38.15 Anti-pattern inventory

- 5,000-line `main.py`.
- Global app imported/mutated from every module.
- Business services require `Request`/`HTTPException` everywhere.
- Every database table gets a public CRUD router automatically.
- All models in one file.
- Dynamic route mutation during normal request handling.
- `app.state` accessed directly throughout domain code.
- Dependency injection used as a universal service locator.

## 38.16 Agent checklist

```text
[ ] Keep app factory/composition root small.
[ ] Group routes by product/domain capability.
[ ] Keep HTTP dependencies at adapter boundary.
[ ] Keep business services independent of FastAPI Request/HTTPException where possible.
[ ] Own pools/clients in lifespan and expose via narrow dependencies.
[ ] Register routers centrally and deterministically.
[ ] Keep persistence and API models separate when their contracts differ.
[ ] Layer tests by domain/integration/API responsibilities.
```

### Sources

- Bigger applications: https://fastapi.tiangolo.com/tutorial/bigger-applications/
- Dependencies: https://fastapi.tiangolo.com/tutorial/dependencies/
- Lifespan: https://fastapi.tiangolo.com/advanced/events/
- APIRouter: https://fastapi.tiangolo.com/reference/apirouter/

---

# FastAPI Advanced — 39) Observability: logging, traces, metrics, request IDs, and diagnostics

## 39.0 Observability mental model

FastAPI provides request lifecycle hooks through middleware, dependencies, exception handlers, and ASGI integration. It does not prescribe one telemetry backend.

Instrument the boundaries:

```text
request accepted
 -> request ID / trace context
 -> route + method
 -> dependency/auth spans
 -> service/downstream spans
 -> response status + size + duration
 -> exception/error classification
```

## 39.1 Structured logging

Prefer machine-parsable logs:

```json
{
  "level": "INFO",
  "event": "request_complete",
  "request_id": "...",
  "method": "GET",
  "route": "/orders/{order_id}",
  "status": 200,
  "duration_ms": 12.8
}
```

Log the **route template** rather than only raw path for aggregation/cardinality control.

## 39.2 Request IDs

Middleware can establish a request ID and expose it through `request.state` or a context variable.

```python
from contextvars import ContextVar
from uuid import uuid4

request_id_var: ContextVar[str | None] = ContextVar(
    "request_id", default=None
)

@app.middleware("http")
async def request_context(request: Request, call_next):
    request_id = request.headers.get("X-Request-ID") or str(uuid4())
    token = request_id_var.set(request_id)
    try:
        response = await call_next(request)
        response.headers["X-Request-ID"] = request_id
        return response
    finally:
        request_id_var.reset(token)
```

Validate external IDs before propagating them.

## 39.3 Access logs vs application logs

ASGI server access logs answer:

```text
request -> status -> basic latency/client
```

Application logs answer:

```text
what business/dependency/downstream event happened and why
```

Use both; do not put domain detail into every generic access log line.

## 39.4 Tracing

OpenTelemetry and vendor APM integrations can instrument ASGI/FastAPI and downstream clients.

Trace high-value spans:

- route request;
- authentication/authorization;
- database query/transaction;
- outbound HTTP/RPC;
- queue publish;
- large serialization/stream setup when relevant.

Avoid a span for every trivial Python helper; useful traces preserve causal structure without excessive overhead/cardinality.

## 39.5 Metrics

Core RED metrics:

```text
Rate
Errors
Duration
```

Service metrics:

- active requests;
- active SSE/WebSocket connections;
- request/response bytes;
- DB pool in-use/wait;
- outbound HTTP pool/wait;
- background job enqueue/failure;
- worker RSS/CPU;
- event-loop lag where measured.

## 39.6 Metric cardinality

Never label metrics directly with:

- user ID;
- order ID;
- arbitrary path;
- request ID;
- raw exception message.

Use bounded dimensions such as route template, method, status family, deployment, and error code.

## 39.7 Exception telemetry

Exception handlers can log safe structured fields while preserving client-safe responses.

Unexpected 500s should capture stack traces server-side. Expected 4xx business/validation outcomes should usually be lower-severity and not flood error alerting.

## 39.8 Dependency timing

Because dependencies form a graph, coarse spans around expensive dependencies—authentication provider, DB session acquisition, permissions call—can isolate latency without instrumenting FastAPI internals.

## 39.9 Streaming telemetry

For streams distinguish:

```text
request accepted
first byte / first event latency
stream duration
items/events sent
bytes sent
client cancellation/disconnect
terminal error
```

A 30-minute SSE connection should not be represented only by a single "30-minute request latency" metric without stream-specific interpretation.

## 39.10 WebSocket telemetry

Useful measurements:

- connections opened/closed;
- current active connections;
- authentication failures;
- inbound/outbound message rate;
- queue depth/drops;
- disconnect close codes;
- broker fan-out latency.

## 39.11 Health/diagnostic endpoints

Keep normal health endpoints simple. Rich diagnostics should be separately authenticated/internal because they can expose dependency names, versions, configuration, or infrastructure topology.

## 39.12 OpenAPI/startup diagnostics

For very large apps, measure startup and OpenAPI generation time. Current 0.140.x improvements specifically target dependency memory/OpenAPI work, so these metrics are useful when validating upgrades.

## 39.13 Privacy

Telemetry is data handling. Establish redaction/classification for:

- request/response bodies;
- query strings;
- authorization headers;
- cookies;
- customer identifiers;
- file names;
- SQL parameters.

Default to metadata, not payload capture.

## 39.14 Anti-pattern inventory

- Logging every request body.
- Metrics labeled with raw URL including IDs.
- Request ID treated as trusted arbitrary log text.
- Expected 404/422 counted as server exceptions indiscriminately.
- Trace spans for every helper call.
- No stream-specific metrics for SSE/WebSockets.
- Liveness endpoint exposing deep system internals.
- No correlation between client error response and server exception log.

## 39.15 Agent checklist

```text
[ ] Establish request/trace correlation at middleware boundary.
[ ] Use structured logs with route templates and stable error codes.
[ ] Collect rate/error/duration plus downstream pool metrics.
[ ] Bound metric label cardinality.
[ ] Trace expensive external/dependency boundaries.
[ ] Add streaming/WebSocket-specific lifecycle metrics.
[ ] Redact credentials and sensitive payload data.
[ ] Correlate safe client request_id with detailed server diagnostics.
```

### Sources

- Middleware: https://fastapi.tiangolo.com/tutorial/middleware/
- Handling errors: https://fastapi.tiangolo.com/tutorial/handling-errors/
- Request reference: https://fastapi.tiangolo.com/reference/request/
- OpenTelemetry FastAPI instrumentation: https://opentelemetry-python-contrib.readthedocs.io/en/latest/instrumentation/fastapi/fastapi.html
- Uvicorn logging: https://www.uvicorn.org/settings/#logging

---

# FastAPI Advanced — 40) API stability, Python/Pydantic/Starlette coupling, and upgrade discipline

## 40.0 Version stance

This reference is pinned to **FastAPI 0.141.1**, released **2026-07-29**.

FastAPI is still on the 0.x version line, and recent releases have contained both substantial features and explicit breaking/internal architecture changes. Production applications should pin dependencies and perform release-note + contract regression review during upgrades.

## 40.1 Python requirement

FastAPI 0.141.1 package metadata requires Python **>=3.10**.

FastAPI 0.136.0 added support for free-threaded Python 3.14t, but support does not imply every application dependency is compatible with free-threaded execution.

## 40.2 FastAPI is a stack

Core relationships:

```text
FastAPI
  -> Starlette: ASGI routing, Request/Response, middleware, TestClient foundations
  -> Pydantic: validation/schema/serialization model layer
  -> typing/Annotated: parameter + dependency declarations
  -> Uvicorn/ASGI server: common runtime/deployment
```

An upgrade can change behavior through any layer even if your own FastAPI imports are unchanged.

## 40.3 Pydantic coupling

Current FastAPI is fundamentally Pydantic v2-oriented. Release 0.135.2 raised the Pydantic lower bound to >=2.9.0.

Upgrade test areas:

- model validation;
- aliases;
- default/unset/null behavior;
- JSON schema;
- serializers/computed fields;
- response validation;
- settings.

## 40.4 Starlette coupling

FastAPI re-exports/conveniences many Starlette primitives, but Starlette is independently versioned.

Changes can affect:

- routing;
- middleware;
- responses;
- WebSockets;
- TestClient;
- multipart/file handling;
- exception groups/lifespan.

Pin through a lockfile rather than manually forcing a Starlette version outside FastAPI's supported range.

## 40.5 Do not depend on private internals

Highest-risk examples:

- manipulating `router.routes` internals;
- importing underscored modules/functions;
- assuming dependency graph internal representation;
- monkey-patching request handler internals.

FastAPI 0.137 is a concrete example: `router.routes` changed from a flat-route expectation into a tree containing intermediate objects and is explicitly described as an internal implementation detail.

## 40.6 Public advanced extension points

Prefer documented extension surfaces:

- `APIRoute.get_route_handler()`;
- `APIRouter(route_class=...)`;
- middleware;
- dependencies;
- exception handlers;
- `app.openapi` override;
- lifespan;
- ASGI mounting.

Even public advanced points deserve upgrade tests.

## 40.7 Pinning with uv

Representative exact project pin:

```bash
uv add 'fastapi[standard]==0.141.1'
```

Commit `uv.lock` and reproduce with frozen/sync semantics in CI/deployment.

Do not manually pin every transitive package unless you have a specific compatibility/security reason; let the lockfile capture a solver-valid graph.

## 40.8 Upgrade workflow

```text
1. read FastAPI release notes across skipped versions
2. inspect Pydantic/Starlette/Uvicorn changes as relevant
3. update lockfile on a branch
4. run type checking
5. run unit/integration/API tests
6. generate OpenAPI before/after and semantic-diff
7. run large-app startup/OpenAPI/memory benchmark if applicable
8. load-test critical endpoints/streams
9. canary/staging deploy
10. monitor latency/error/memory before broad rollout
```

## 40.9 Contract tests

Keep explicit tests for:

- status codes;
- JSON schema/shape;
- content types;
- headers/cookies;
- auth challenges/scopes;
- request validation error structure if clients depend on it;
- operation IDs;
- OpenAPI security declarations.

## 40.10 Current router migration risk

If upgrading from <0.137:

Search codebase for:

```text
.routes
APIRoute isinstance loops
route mutation after inclusion
custom router flattening
OpenAPI utilities that inspect raw router.routes
```

Use current documented APIs, including `iter_route_contexts()` where appropriate for advanced introspection.

## 40.11 Current streaming migration risk

If upgrading across 0.134–0.140:

Test:

- typed generator return behavior;
- JSONL/SSE status codes;
- SSE line framing;
- `include_router()` preserving stream item type;
- iterable response-model options;
- client cancellation.

Several 0.140 patch releases specifically fixed these surfaces.

## 40.12 Current dependency-memory migration opportunity

If an older codebase introduced custom caching/flattening or avoided dependency composition because of memory/OpenAPI costs, retest on 0.141.1 before preserving the workaround. FastAPI 0.140.x substantially optimized these exact paths.

## 40.13 Deprecations

Current FastAPI documentation recommends lifespan over the alternative startup/shutdown event mechanism for new code.

When release notes or reference docs mark an API deprecated:

```text
new code -> use replacement immediately
existing code -> schedule migration before removal
agent code generation -> never emit deprecated form unless maintaining legacy code
```

## 40.14 Anti-pattern inventory

- `fastapi = "*"` / floating unreviewed upgrades.
- Pinning FastAPI but not committing dependency lock.
- Forcing incompatible Starlette/Pydantic versions.
- Depending on router/dependency private internals.
- Upgrade without OpenAPI diff.
- Ignoring patch releases because "only patch"; 0.140 patch releases contained meaningful streaming/correctness fixes.
- Adopting free-threaded Python without testing full dependency stack.

## 40.15 Agent checklist

```text
[ ] Pin FastAPI 0.141.1 for this reference target.
[ ] Commit a complete solved lockfile.
[ ] Treat Starlette/Pydantic as material behavior dependencies.
[ ] Avoid private router/dependency internals.
[ ] Read every skipped release note during upgrade.
[ ] Diff OpenAPI and run contract tests.
[ ] Retest streams/router customization/large dependency graphs specifically.
[ ] Validate full ecosystem before free-threaded Python adoption.
```

### Sources

- Release notes: https://fastapi.tiangolo.com/release-notes/
- FastAPI package: https://pypi.org/project/fastapi/
- FastAPI reference: https://fastapi.tiangolo.com/reference/fastapi/
- About versions: https://fastapi.tiangolo.com/deployment/versions/
- Pydantic migration/docs: https://docs.pydantic.dev/latest/
- Starlette: https://www.starlette.io/

---

# FastAPI Advanced — 41) Release delta: FastAPI 0.134.0 through 0.141.1

## 41.0 Why this release window matters

The February–July 2026 release series materially changed FastAPI's capability surface and internals. An older reference that stops before this window misses first-class streaming, SSE, free-threaded Python support, live router-tree semantics, frontend serving, and substantial dependency memory/OpenAPI optimization.

## 41.1 0.134.0 — 2026-02-27

Primary feature: **streaming JSON Lines and binary data with `yield`**.

Implications:

- generator/async-generator endpoint returns became a first-class FastAPI streaming surface;
- JSON Lines can be typed/validated/serialized rather than manually framed;
- binary streaming with yield integrates with endpoint semantics;
- Starlette lower-bound upgrade accompanied exception-group behavior needed for the feature.

Agent migration rule: prefer first-class typed streaming when it matches the response instead of hand-writing `StreamingResponse` wrappers for every record stream.

## 41.2 0.135.0 — 2026-03-01

Primary feature: **Server-Sent Events (SSE)**.

FastAPI gained a dedicated SSE surface (`fastapi.sse`, `EventSourceResponse`) with typed iterable/generator integration.

Use for one-way server event streams where EventSource/SSE semantics are appropriate.

## 41.3 0.135.1 — 2026-03-01

Important correctness fix around yield dependencies and TaskGroup/async context-manager cleanup.

Lesson: dependency cleanup semantics are runtime-critical; do not build your own assumptions about how generator dependencies are scheduled beyond documented scope/lifecycle behavior.

## 41.4 0.135.2 — 2026-03-01

Pydantic lower bound increased to **>=2.9.0**.

This makes Pydantic v2 behavior a firmer baseline for the current FastAPI reference.

## 41.5 0.136.0 — 2026-04-16

Added support for **free-threaded Python 3.14t**.

This is compatibility enablement, not an application-level guarantee that every dependency or mutable global is safe under free-threaded concurrency.

## 41.6 0.136.2 — 2026-05-23

Strengthened SSE field validation to prevent applications from emitting malformed SSE data.

## 41.7 0.136.3 — 2026-05-23

Changed header underscore behavior: with `convert_underscores=True` (default), underscore header forms are not accepted as equivalent to the normalized hyphen form.

Migration test: clients sending nonstandard underscore headers should be corrected.

## 41.8 0.137.0 — 2026-06-14

Major **breaking internal architecture refactor**: preserve `APIRouter` and `APIRoute` instances instead of cloning/recreating included path operations.

Key current semantics:

- router inclusion forms a live tree;
- routes added after inclusion can be reflected;
- subrouters can be included before their routes are added;
- custom route/router instances survive inclusion;
- some memory can be saved by avoiding copies.

Breaking detail:

```text
router.routes is no longer safely modeled as a flat list of final APIRoute objects
```

The release notes explicitly classify direct `router.routes` iteration/mutation as affected and tell users to treat it as internal implementation detail.

The release also exposed experimental/alpha router customization methods; because the release notes call them not officially supported, do not make them a stable production dependency without current-reference verification.

## 41.9 0.137.2 — 2026-06-18

Added `iter_route_contexts()` for advanced cases that previously inspected `router.routes` directly.

This is the migration direction for advanced tooling that needs flattened/derived route context while respecting the live tree.

## 41.10 0.138.0 — 2026-06-20

Added first-class frontend serving:

```python
app.frontend("/", directory="dist")
router.frontend("/", directory="dist")
```

This creates a FastAPI-native static frontend/SPA integration distinct from raw `StaticFiles` mounting.

## 41.11 0.138.2 — 2026-06-29

Frontend fallback was tightened: methods other than `GET`/`HEAD` with no static-file match return 404 instead of incorrectly receiving frontend fallback behavior.

This is a correctness/security-relevant routing semantic for SPA deployments.

## 41.12 0.139.0 — 2026-07-01

Added dependencies to `app.frontend()`, enabling FastAPI dependency policy (for example cookie/session setup/auth) to participate in frontend responses.

## 41.13 0.139.2 — 2026-07-16

Router route-building refactor improved thread safety, primarily relevant to unusual parallel-thread test/build scenarios.

## 41.14 0.140.0 — 2026-07-24

Core refactor: **reduce memory usage in dependencies**.

This began a concentrated optimization series for large dependency graphs and OpenAPI generation.

## 41.15 0.140.1 — 2026-07-27

Adjusted dependency LRU cache limit to account for large applications.

## 41.16 0.140.2 — 2026-07-27

Stopped retaining flat dependency trees.

This reduces memory pressure and reinforces that flattened dependency representations are internal implementation details, not application APIs.

## 41.17 0.140.3 — 2026-07-27

Avoided repeated dependency flattening during OpenAPI generation.

## 41.18 0.140.4 — 2026-07-27

Skipped unused dependency repeat bookkeeping.

## 41.19 0.140.5 — 2026-07-27

Avoided flattening dependencies for body fields.

## 41.20 0.140.6 — 2026-07-27

Avoided flattening dependencies for request parameters, especially for OpenAPI.

## 41.21 0.140.7 — 2026-07-27

Avoided flattening dependencies for OpenAPI.

The 0.140.0–0.140.7 series should be treated collectively as a **large-app dependency-memory and OpenAPI performance architecture change**.

## 41.22 0.140.8 — 2026-07-28

Fixed stream item type being lost when the streaming endpoint lived under `include_router()`.

Regression test important for typed JSONL/SSE route composition.

## 41.23 0.140.9 — 2026-07-28

Fixed `exclude_defaults` propagation into dictionary keys/values in `jsonable_encoder`.

## 41.24 0.140.10 — 2026-07-28

Fixed handling of sequences with nested `Annotated` types.

## 41.25 0.140.11 — 2026-07-28

Fixed `response_model_*` parameters being ignored for non-generator endpoints with `Iterable[...]` return annotations.

## 41.26 0.140.12 — 2026-07-28

Fixed SSE event line splitting to comply with the SSE specification.

## 41.27 0.140.13 — 2026-07-28

Fixed explicit `status_code` being ignored for SSE and JSON Lines streaming endpoints.

Also added dedicated API reference documentation for `fastapi.sse`.

## 41.28 0.141.0 — 2026-07-29

Added:

```python
app.frontend(..., check_dir="auto")
```

This improves local development with `fastapi dev` by making missing frontend build-directory behavior environment-aware.

## 41.29 0.141.1 — 2026-07-29

Current stable target.

Fixes:

- background tasks and headers from dependencies now work correctly with `app.frontend()`;
- docs clarify `FASTAPI_ENV` in FastAPI CLI guidance.

## 41.30 Upgrade priorities by source version

### From <=0.133

Review:

```text
first-class yield streaming
JSON Lines
SSE
Pydantic >=2.9 baseline
free-threaded Python compatibility
router-tree architecture
frontend APIs
dependency-memory refactors
strict current streaming fixes
```

### From 0.134–0.136

Focus:

```text
router.routes assumptions
live router inclusion
frontend serving
dependency memory/OpenAPI performance
streaming patch fixes
```

### From 0.137–0.139

Focus:

```text
0.140 dependency internal refactors
SSE/JSONL patch correctness
frontend check_dir/FASTAPI_ENV behavior
```

### From 0.140.0–0.140.7

Upgrade to 0.141.1 rather than stopping mid-patch series if your constraints allow; the later patches contain concrete streaming, encoder, iterable, and annotation correctness fixes.

## 41.31 Agent checklist

```text
[ ] Treat 0.134 streaming and 0.135 SSE as first-class APIs.
[ ] Treat 0.137 as a router architecture/breaking-internal boundary.
[ ] Replace direct router.routes assumptions with documented route APIs.
[ ] Use app.frontend for current static SPA serving where applicable.
[ ] Re-benchmark large dependency graphs on 0.141.1.
[ ] Regression-test streamed status codes, SSE framing, router stream typing.
[ ] Pin current stable 0.141.1 rather than an early 0.140 patch when possible.
```

### Source

- FastAPI release notes: https://fastapi.tiangolo.com/release-notes/

---

# FastAPI Advanced — 42) Production architecture patterns

## 42.0 Pattern catalog

This section translates FastAPI primitives into end-to-end application patterns. The goal is not to prescribe one architecture, but to make ownership/lifetime/performance/security consequences explicit.

## 42.1 Pattern A — CRUD/service API with relational database

```text
Ingress/TLS
 -> FastAPI router
   -> authentication dependency
   -> request-scoped DB session/UoW
   -> service
   -> repository
   -> PostgreSQL
```

Recommended primitives:

- Pydantic request/response models;
- router-level tags/security;
- `yield` DB dependency;
- lifespan-owned engine;
- response models that exclude private DB fields;
- pagination bounds;
- DB pool size multiplied by worker count.

## 42.2 Pattern B — internal async aggregation API

```text
request
 -> FastAPI async endpoint
 -> concurrently call several downstream services
 -> aggregate typed models
 -> response model
```

Use:

- lifespan-owned `httpx.AsyncClient`;
- per-call timeout/budget;
- `asyncio.TaskGroup`/structured concurrency where appropriate;
- circuit/bulkhead policy if required;
- trace propagation;
- no new client per request.

## 42.3 Pattern C — browser SPA + API in one service

```text
/api/* routes -> JSON API
/*           -> app.frontend("/", directory="dist")
```

Use current FastAPI frontend integration:

- API route precedence;
- `check_dir="auto"` development behavior;
- frontend dependencies for cookie/session behavior;
- SPA fallback only for navigational GET/HEAD;
- appropriate cache headers for hashed assets;
- browser security headers/CSP;
- cookie/CSRF design when using sessions.

## 42.4 Pattern D — API behind stripped ingress prefix

```text
public: https://example.com/platform/api/...
proxy strips /platform
FastAPI root_path=/platform
routes remain /api/...
```

Use:

- trusted proxy headers;
- root path;
- OpenAPI server verification;
- OAuth redirect tests through public URL;
- no hardcoded prefix in every router.

## 42.5 Pattern E — large modular monolith

```text
one FastAPI process artifact
  -> many APIRouter modules
  -> shared app lifespan resources
  -> domain/service boundaries
  -> one OpenAPI contract
```

Current 0.137+ live-router architecture supports clean hierarchical composition without copying routes. Do not inspect/mutate `router.routes` internals.

## 42.6 Pattern F — high-volume read API

Goals:

```text
low latency
bounded output
connection reuse
minimal DB round trips
```

Techniques:

- async or correctly offloaded sync DB stack;
- query projection/preloading;
- bounded pagination;
- response models unless measured bottleneck;
- cached read model where semantics allow;
- profile serialization only after DB latency is controlled;
- multiple replicas/workers within DB capacity.

## 42.7 Pattern G — large export / record stream

```text
request
 -> authorize export
 -> query/read in chunks
 -> JSONL/binary streaming
 -> proxy streaming
```

Prefer first-class JSON Lines when emitting structured records.

Avoid:

```python
rows = await fetch_all_50_million()
return rows
```

Design client cancellation, timeouts, and database transaction lifetime.

## 42.8 Pattern H — server event stream

```text
client EventSource
 -> FastAPI EventSourceResponse
 -> async generator
 -> broker subscription
```

Use:

- first-class SSE;
- per-client bounded queue;
- broker for cross-worker events;
- heartbeat/idle policy;
- proxy buffering disabled;
- disconnect cleanup;
- event ID/reconnect semantics when product requires replay.

## 42.9 Pattern I — bidirectional real-time app

```text
browser WebSocket
 -> FastAPI WebSocket route
 -> connection manager
 -> broker/pubsub
 -> other workers/services
```

Use WebSockets instead of SSE when client-to-server messages are continuous and bidirectional.

Design:

- handshake auth;
- message schemas;
- backpressure;
- disconnect cleanup;
- shared broker;
- authorization re-check for privileged events.

## 42.10 Pattern J — long-running job API

```text
POST /jobs
 -> validate + authorize
 -> durable enqueue/persist
 -> 202 + job_id

worker
 -> process
 -> persist status/result

GET /jobs/{id}
 -> status/result
```

Do **not** use in-process `BackgroundTasks` for work that must survive process crash/redeploy.

Use BackgroundTasks for small best-effort after-response work such as noncritical logging/notification when loss semantics are acceptable.

## 42.11 Pattern K — ML inference API

```text
lifespan -> load model once per worker
request -> validate tensor/image metadata
 -> bounded preprocessing
 -> inference
 -> response
```

Capacity implications:

```text
model 4 GiB x 4 workers = ~16 GiB model memory
```

Possible design:

- one worker per GPU/process;
- separate inference service;
- batching queue;
- process pool for CPU inference;
- streaming output only when model semantics benefit.

Do not create/load model per request.

## 42.12 Pattern L — multi-tenant API

```text
credential
 -> principal
 -> allowed tenants
request tenant
 -> validated TenantContext
 -> repository automatically scopes every query
```

Use dependencies for tenant context but enforce tenant filters again at persistence/repository boundary where feasible.

Do not let a caller-controlled header alone decide database schema/bucket/credential selection.

## 42.13 Pattern M — public generated-SDK API

Priorities:

- stable operation IDs;
- explicit response/error models;
- versioned compatibility policy;
- OpenAPI semantic diff in CI;
- tags/names for generated client organization;
- additive model evolution review;
- deprecation before removal.

Treat OpenAPI as a published artifact.

## 42.14 Pattern N — internal administrative API

Use:

- separate router prefix/tag;
- strong auth dependency and network policy;
- `include_in_schema` decision deliberate;
- audit logs;
- no assumption that obscurity is authorization;
- separate rate limits and destructive-operation controls.

## 42.15 Pattern O — webhook receiver

```text
POST /webhooks/provider
 -> verify signature against raw payload
 -> parse/validate event
 -> idempotency/deduplication
 -> enqueue/process
 -> quick acknowledgement
```

Be careful: signature verification may need exact raw body bytes before model transformation. Use `Request` or custom dependency/route behavior deliberately.

## 42.16 Pattern P — outbound webhook publisher

Use FastAPI OpenAPI `webhooks` to document outbound payload contracts where clients need a schema.

Runtime delivery needs:

- durable retry queue;
- signed payloads;
- timeout/backoff;
- idempotency/event IDs;
- endpoint validation/SSRF policy;
- observability.

## 42.17 Pattern Q — microservice with one container process

```text
Kubernetes Deployment
  replicas=N
  each pod:
    one fastapi run/uvicorn process
    one DB/HTTP client pool
```

This is the simplest orchestration-aligned scaling shape.

## 42.18 Pattern R — single VM, multi-worker

```text
systemd/container
 -> fastapi run --workers N
```

Good when no cluster scheduler exists. Ensure:

- external restart supervision;
- TLS/reverse proxy;
- DB pool multiplication;
- shared state outside worker memory;
- migration pre-step once.

## 42.19 Pattern S — FastAPI subapplications

Use `mount()` only when you truly need separate ASGI applications, e.g.:

- independent docs/OpenAPI;
- legacy WSGI bridge;
- separately configured admin app;
- third-party ASGI app.

Use `include_router()` for normal modular API composition.

## 42.20 Pattern T — contract-first compatibility gateway

```text
external OpenAPI contract
 -> FastAPI adapter routes/models
 -> normalized internal service interface
```

Pin explicit operation IDs and response schemas. Test exact status/header/content semantics. Keep external quirks at the adapter layer rather than polluting core services.

## 42.21 Architecture selection matrix

| Requirement | Strong default |
|---|---|
| simple typed HTTP API | FastAPI + Pydantic + Uvicorn |
| static SPA + API | `app.frontend()` + API routes |
| outbound one-way live events | SSE |
| bidirectional realtime | WebSockets |
| huge structured record output | JSON Lines streaming |
| durable long job | queue + worker, not BackgroundTasks |
| modular one API | `APIRouter` + `include_router()` |
| independent ASGI app | `mount()` |
| request resource cleanup | `yield` dependency |
| app resource lifetime | lifespan |
| cluster scaling | replicas, usually one process/container |
| single-host CPU scaling | multiple workers |

## 42.22 Agent checklist

```text
[ ] Match transport primitive to semantics: HTTP / JSONL / SSE / WebSocket.
[ ] Match lifetime: request dependency / app lifespan / external durable worker.
[ ] Match composition: router vs mount.
[ ] Match scaling: worker vs replica vs job worker.
[ ] Keep process-local memory assumptions explicit.
[ ] Preserve OpenAPI contract for external/generated clients.
[ ] Put shared state and durable work outside worker process when required.
```

### Sources

- FastAPI tutorials/reference: https://fastapi.tiangolo.com/
- Deployment concepts: https://fastapi.tiangolo.com/deployment/concepts/
- JSON Lines: https://fastapi.tiangolo.com/tutorial/stream-json-lines/
- SSE: https://fastapi.tiangolo.com/tutorial/server-sent-events/
- WebSockets: https://fastapi.tiangolo.com/advanced/websockets/
- Frontend: https://fastapi.tiangolo.com/tutorial/frontend/
- Bigger applications: https://fastapi.tiangolo.com/tutorial/bigger-applications/

---
# FastAPI Advanced — 43) Dense appendices and lookup matrices

This section is intentionally lookup-first. Use it when an agent already understands the architecture and needs the shortest route to the current stable syntax/decision rule.

## 43.1 Current version card

```text
Framework:       FastAPI
Reference pin:   0.141.1
Release date:    2026-07-29
Python:          >=3.10
Core model:      ASGI + Starlette + Pydantic v2
Dev launcher:    fastapi dev
Prod launcher:   fastapi run / uvicorn
Schema:          OpenAPI + JSON Schema
Current major recent additions:
  0.134  typed yield streaming / JSON Lines
  0.135  SSE
  0.136  Python 3.14t support
  0.137  live router tree / preserve APIRouter/APIRoute
  0.138  app.frontend()/router.frontend()
  0.139  frontend dependencies
  0.140  dependency memory/OpenAPI optimization series
  0.141  frontend check_dir="auto" + fixes
```

## 43.2 Installation matrix

| Goal | Command | Notes |
|---|---|---|
| recommended full dev/runtime | `uv add 'fastapi[standard]==0.141.1'` | includes CLI and common optional stack |
| FastAPI core only | `uv add 'fastapi==0.141.1'` | add server/extras explicitly |
| core + Uvicorn | `uv add 'fastapi==0.141.1' uvicorn` | intentionally smaller surface |
| settings | `uv add pydantic-settings` | optional |
| forms/files without standard extra | `uv add python-multipart` | required for multipart/form handling |
| templates without standard extra | `uv add jinja2` | optional |
| testing without standard extra | `uv add --dev httpx pytest` | HTTPX used by test stack |
| async tests | add AnyIO/pytest integration as needed | FastAPI docs use `pytest.mark.anyio` |

Production rule:

```text
pyproject.toml pin + uv.lock commit + frozen CI/deploy sync
```

## 43.3 Canonical `pyproject.toml`

```toml
[project]
name = "example-api"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = [
  "fastapi[standard]==0.141.1",
  "pydantic-settings",
]

[tool.fastapi]
entrypoint = "app.main:app"
```

For applications rather than reusable libraries, commit the resolved lockfile.

## 43.4 Current `FastAPI(...)` constructor surface

Current reference signature, formatted for lookup:

```python
FastAPI(
    *,
    debug=False,
    routes=None,
    title="FastAPI",
    summary=None,
    description="",
    version="0.1.0",
    openapi_url="/openapi.json",
    openapi_tags=None,
    servers=None,
    dependencies=None,
    default_response_class=Default(JSONResponse),
    redirect_slashes=True,
    docs_url="/docs",
    redoc_url="/redoc",
    swagger_ui_oauth2_redirect_url="/docs/oauth2-redirect",
    swagger_ui_init_oauth=None,
    middleware=None,
    exception_handlers=None,
    on_startup=None,
    on_shutdown=None,
    lifespan=None,
    terms_of_service=None,
    contact=None,
    license_info=None,
    openapi_prefix="",
    root_path="",
    root_path_in_servers=True,
    responses=None,
    callbacks=None,
    webhooks=None,
    deprecated=None,
    include_in_schema=True,
    swagger_ui_parameters=None,
    generate_unique_id_function=Default(generate_unique_id),
    separate_input_output_schemas=True,
    openapi_external_docs=None,
    strict_content_type=True,
    **extra,
)
```

### High-value constructor groups

| Group | Fields |
|---|---|
| identity/docs | `title`, `summary`, `description`, `version`, `terms_of_service`, `contact`, `license_info` |
| OpenAPI | `openapi_url`, `openapi_tags`, `servers`, `callbacks`, `webhooks`, `responses`, `generate_unique_id_function`, `separate_input_output_schemas`, `openapi_external_docs` |
| docs UI | `docs_url`, `redoc_url`, `swagger_ui_oauth2_redirect_url`, `swagger_ui_init_oauth`, `swagger_ui_parameters` |
| policy | `dependencies`, `default_response_class`, `redirect_slashes`, `deprecated`, `include_in_schema`, `strict_content_type` |
| runtime/app | `middleware`, `exception_handlers`, `lifespan` |
| deployment path | `root_path`, `root_path_in_servers` |
| legacy/compat | `on_startup`, `on_shutdown`, `openapi_prefix` | prefer current documented alternatives for new code |

Agent default:

```python
app = FastAPI(
    title="Product API",
    version="1.0.0",
    lifespan=lifespan,
)
```

Keep `strict_content_type=True` unless there is an explicit compatibility case.

## 43.5 Path operation decorator matrix

| HTTP method | Decorator |
|---|---|
| GET | `@app.get(path, ...)` / `@router.get(...)` |
| POST | `@app.post(...)` |
| PUT | `@app.put(...)` |
| PATCH | `@app.patch(...)` |
| DELETE | `@app.delete(...)` |
| OPTIONS | `@app.options(...)` |
| HEAD | `@app.head(...)` |
| TRACE | `@app.trace(...)` |
| WebSocket | `@app.websocket(path, ...)` |

Common path-operation metadata:

```python
@app.post(
    "/items",
    response_model=ItemPublic,
    status_code=201,
    tags=["items"],
    summary="Create item",
    description="Create one item.",
    response_description="Created item",
    responses={409: {"model": ErrorResponse}},
    deprecated=False,
    operation_id="items_create",
    include_in_schema=True,
    dependencies=[Depends(require_write_access)],
)
async def create_item(...):
    ...
```

## 43.6 Parameter-source inference matrix

| Function parameter shape | Default FastAPI source |
|---|---|
| name appears in route path | path |
| scalar type not in path | query |
| Pydantic model | request body |
| `Annotated[..., Path(...)]` | path |
| `Annotated[..., Query(...)]` | query |
| `Annotated[..., Header(...)]` | header |
| `Annotated[..., Cookie(...)]` | cookie |
| `Annotated[..., Body(...)]` | body |
| `Annotated[..., Form(...)]` | form body |
| `Annotated[..., File(...)]` | multipart file |
| `UploadFile` with `File()` | multipart uploaded file |
| `Annotated[..., Depends(...)]` | dependency result, not caller field |
| `Request` | Starlette request object |
| `Response` | mutable response metadata object |
| `BackgroundTasks` | background-task collector |
| `WebSocket` in websocket route | WebSocket connection |

Core inference invariant:

```text
path-name match wins for path parameters;
plain singular scalar -> query;
Pydantic model -> body;
explicit marker -> marker source.
```

## 43.7 Preferred `Annotated` syntax

```python
from typing import Annotated
from fastapi import Body, Cookie, Depends, File, Form, Header, Path, Query

ItemId = Annotated[int, Path(gt=0)]
Search = Annotated[str | None, Query(max_length=100)]
Token = Annotated[str, Header(alias="X-Token")]
SessionId = Annotated[str | None, Cookie(alias="session_id")]
Payload = Annotated[ItemCreate, Body()]
Username = Annotated[str, Form()]
Upload = Annotated[UploadFile, File()]
UserDep = Annotated[User, Depends(get_current_user)]
```

Prefer `Annotated` for current code because it keeps the Python default distinct from FastAPI metadata and composes cleanly with type aliases.

## 43.8 Common parameter-marker arguments

Across `Query`, `Path`, `Header`, `Cookie`, `Body`, `Form`, and `File`, common categories include:

| Category | Typical fields |
|---|---|
| naming | `alias`, `alias_priority`, validation/serialization aliases where supported |
| docs | `title`, `description`, `examples`, `deprecated` |
| numeric constraints | `gt`, `ge`, `lt`, `le`, `multiple_of` |
| string constraints | `min_length`, `max_length`, `pattern` |
| schema | `json_schema_extra` |
| body/form/file | `media_type` and source-specific behavior |

Use Pydantic `Field(...)` inside models for model-field validation; use FastAPI parameter markers to declare transport source + request-level metadata.

## 43.9 Header normalization rule

Default:

```python
user_agent: Annotated[str | None, Header()] = None
```

maps Python `user_agent` to HTTP `user-agent`.

Current release-line rule after 0.136.3: with default underscore conversion, do not expect a caller's literal underscore header variant to be accepted as equivalent.

If wire name is contractual, use explicit alias:

```python
request_id: Annotated[str, Header(alias="X-Request-ID")]
```

## 43.10 Query/header/cookie parameter models

Current FastAPI supports Pydantic models for grouped query/header/cookie parameters.

```python
class FilterParams(BaseModel):
    limit: int = Field(100, gt=0, le=100)
    offset: int = Field(0, ge=0)

@app.get("/items")
async def list_items(filters: Annotated[FilterParams, Query()]):
    return filters
```

Use when a coherent group repeats and deserves one typed contract.

## 43.11 Request-body patterns

### Single body model

```python
@app.post("/items")
async def create_item(item: ItemCreate):
    ...
```

### Explicit body marker

```python
item: Annotated[ItemCreate, Body()]
```

### Embedded body

```python
item: Annotated[ItemCreate, Body(embed=True)]
```

Conceptual JSON:

```json
{"item": {"name": "x"}}
```

rather than:

```json
{"name": "x"}
```

### Multiple body values

FastAPI combines explicitly declared body values/models into one request object schema.

## 43.12 Form/file rules

```text
application/x-www-form-urlencoded -> Form
multipart/form-data               -> Form + File / UploadFile
application/json                  -> Body / Pydantic model
```

Protocol invariant:

```text
one HTTP request body has one encoding;
do not expect JSON Body fields and multipart Form/File fields simultaneously.
```

## 43.13 `bytes` vs `UploadFile`

| Type | Behavior/use |
|---|---|
| `bytes` | whole uploaded content presented as bytes; convenient for small files |
| `UploadFile` | file-like/spooled upload interface; better default for larger files |

Canonical:

```python
@app.post("/upload")
async def upload(file: Annotated[UploadFile, File()]):
    data = await file.read(1024)
    ...
```

Do not trust `filename` or `content_type` as security truth.

## 43.14 `Depends()` exact current signature

```python
Depends(
    dependency=None,
    *,
    use_cache=True,
    scope=None,
)
```

Meaning:

| Field | Default | Meaning |
|---|---:|---|
| `dependency` | `None` | callable FastAPI invokes |
| `use_cache` | `True` | reuse result when dependency appears repeatedly in one request graph |
| `scope` | `None` | primarily controls `yield` dependency teardown lifetime |

Typical:

```python
CurrentUser = Annotated[User, Depends(get_current_user)]
```

Do not call the dependency yourself inside `Depends`:

```python
Depends(get_current_user)    # correct
Depends(get_current_user())  # wrong
```

## 43.15 `Security()` exact current signature

```python
Security(
    dependency=None,
    *,
    scopes=None,
    use_cache=True,
)
```

Use instead of `Depends` when required OAuth2 scopes must appear in OpenAPI/security context:

```python
CurrentUser = Annotated[
    User,
    Security(get_current_user, scopes=["items:read"]),
]
```

OAuth scopes are declarative/authz inputs, not a replacement for resource/tenant policy.

## 43.16 Dependency cache truth table

| Same dependency appears twice in one request | `use_cache=True` | `use_cache=False` |
|---|---|---|
| execution count | normally once | can execute again |
| returned object | shared cached instance | fresh result |
| DB session/auth principal | usually desired | rarely |
| nonce/current-time/random fresh value | maybe wrong | often relevant |

Cache scope is request dependency resolution, not a cross-request cache.

## 43.17 Yield dependency scope truth table

| Scope | Setup | Endpoint | Teardown | Stream can still be sending after teardown? |
|---|---|---|---|---|
| `"function"` | before handler | executes | after handler returns, before response sent | yes |
| `"request"` | before handler | executes | after response cycle | no, teardown waits until response cycle ends |
| `None` | inferred/default rules | executes | framework default based on dependency graph | verify when resource lifetime matters |

Use `scope="function"` when a resource can be released immediately after producing the response object and is not needed during response streaming.

Use request lifetime when the response/stream still requires the resource.

## 43.18 Yield dependency nesting rule

A shorter-lived dependency cannot safely outlive the resources it depends on.

Design nested scopes so parent/subdependency teardown order remains valid.

Database example:

```text
engine/client -> app lifespan
session       -> request/function dependency
transaction   -> service/UoW scope
```

## 43.19 App-level/global dependencies

```python
app = FastAPI(
    dependencies=[
        Depends(verify_platform_header),
    ]
)
```

Router-level:

```python
router = APIRouter(
    dependencies=[Depends(require_internal_access)]
)
```

Path-level:

```python
@app.get(
    "/admin",
    dependencies=[Depends(require_admin)],
)
```

If the endpoint needs the resolved value, inject it as a parameter instead of decorator-only dependency.

## 43.20 Response behavior matrix

| Endpoint returns | FastAPI behavior |
|---|---|
| Pydantic model / dict / dataclass / scalar | normal response conversion/validation according to declared contract |
| value with `response_model=` | filter/validate/serialize against response model |
| `Response` subclass instance | use response directly; bypass normal automatic model conversion for body |
| generator / async generator under first-class stream semantics | stream according to inferred/declared stream response |
| `StreamingResponse` instance | raw chunk streaming; chunks not JSON-converted |
| `EventSourceResponse` / SSE yield | SSE wire formatting |

Security rule:

```text
Response returned directly => you own filtering/serialization/content correctness.
```

## 43.21 `response_model` vs return annotation

### Return annotation as contract

```python
@app.get("/users/{id}")
async def get_user(id: str) -> UserPublic:
    ...
```

### Explicit response model

```python
@app.get("/users/{id}", response_model=UserPublic)
async def get_user(id: str):
    ...
```

Use explicit `response_model` when implementation return type differs from public output type or when you need clearer adapter-layer separation.

## 43.22 Response model filtering options

Common route-level controls:

```text
response_model_include
response_model_exclude
response_model_by_alias
response_model_exclude_unset
response_model_exclude_defaults
response_model_exclude_none
```

Prefer dedicated public output models for stable API design instead of large ad hoc include/exclude sets.

## 43.23 Common response classes

| Class | Use |
|---|---|
| `Response` | raw bytes/text + exact media type |
| `JSONResponse` | explicit JSON response/status/headers |
| `ORJSONResponse` | optional orjson-backed response class; benchmark/only if installed |
| `HTMLResponse` | HTML |
| `PlainTextResponse` | plain text |
| `RedirectResponse` | redirects |
| `StreamingResponse` | raw incremental chunks |
| `FileResponse` | file transfer with file metadata/range behavior from Starlette stack |
| `EventSourceResponse` | SSE (`text/event-stream`) |

Import:

```python
from fastapi.responses import (
    FileResponse,
    HTMLResponse,
    JSONResponse,
    ORJSONResponse,
    PlainTextResponse,
    RedirectResponse,
    Response,
    StreamingResponse,
)
from fastapi.sse import EventSourceResponse, ServerSentEvent
```

## 43.24 Additional status code pattern

```python
@app.put("/items/{item_id}")
async def upsert(item_id: str, item: ItemCreate):
    existing = await get(item_id)
    if existing:
        return update(existing, item)
    created = await create(item_id, item)
    return JSONResponse(
        status_code=201,
        content=created.model_dump(mode="json"),
    )
```

If returning a Response directly, ensure serialization matches the intended contract.

## 43.25 Header/cookie mutation pattern

### Injected `Response`

```python
@app.get("/items")
async def items(response: Response):
    response.headers["X-Trace"] = "..."
    response.set_cookie("session", "...", httponly=True, secure=True)
    return {"items": []}
```

FastAPI can still apply normal output processing to the returned value while taking status/header/cookie metadata from the injected response object.

### Direct response

```python
return JSONResponse(
    content={"items": []},
    headers={"X-Trace": "..."},
)
```

## 43.26 Background task pattern

```python
from fastapi import BackgroundTasks

@app.post("/notify")
async def notify(background_tasks: BackgroundTasks):
    background_tasks.add_task(send_best_effort_notification, "...")
    return {"accepted": True}
```

Use for best-effort in-process after-response work.

Do not use for must-survive-redeploy jobs; enqueue durably instead.

## 43.27 Lifespan canonical syntax

```python
from contextlib import asynccontextmanager
from fastapi import FastAPI

@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.http = await create_http_client()
    app.state.db = await create_pool()
    try:
        yield
    finally:
        await app.state.http.aclose()
        await app.state.db.close()

app = FastAPI(lifespan=lifespan)
```

Use for process/application-owned shared resources.

## 43.28 Lifetime decision matrix

| Lifetime | Primitive |
|---|---|
| one Python expression/call | local variable |
| one endpoint request | dependency |
| request with teardown | yield dependency |
| endpoint function only | yield dependency `scope="function"` |
| full response cycle | yield dependency `scope="request"` |
| one app worker process | lifespan |
| across workers/replicas | external service/storage |
| must survive process crash | durable external store/queue |

## 43.29 JSON Lines current pattern

Conceptual current style:

```python
from collections.abc import AsyncIterable
from pydantic import BaseModel

class Item(BaseModel):
    id: str
    value: int

@app.get("/items.jsonl")
async def stream_items() -> AsyncIterable[Item]:
    for item in source:
        yield Item.model_validate(item)
```

FastAPI's first-class stream path serializes records as JSON Lines based on the typed iterable/yield contract.

Use for record streams, not arbitrary one-off JSON responses.

## 43.30 SSE current pattern

```python
from collections.abc import AsyncIterable
from fastapi.sse import EventSourceResponse, ServerSentEvent

@app.get("/events", response_class=EventSourceResponse)
async def events() -> AsyncIterable[ServerSentEvent]:
    async for event in event_source():
        yield ServerSentEvent(
            data=event.payload,
            event=event.type,
            id=event.id,
        )
```

`EventSourceResponse` current constructor reference:

```python
EventSourceResponse(
    content,
    status_code=200,
    headers=None,
    media_type=None,
    background=None,
)
```

Use plain yielded Pydantic/data values when you only need `data`; yield `ServerSentEvent` when you need `event`, `id`, `retry`, or comments.

## 43.31 Stream primitive decision matrix

| Need | Primitive |
|---|---|
| one JSON object | normal FastAPI response model |
| large sequence of typed records | JSON Lines yield stream |
| server -> client text events / browser EventSource | SSE |
| arbitrary bytes/chunks/file-like stream | `StreamingResponse` |
| static/local file download | `FileResponse` |
| bidirectional live protocol | WebSocket |
| must disconnect then later retrieve | durable async job API |

## 43.32 Streaming safety checklist

```text
[ ] generator is cancellation-aware
[ ] downstream resources have correct lifetime
[ ] no open transaction unnecessarily spans entire stream
[ ] proxy buffering disabled where needed
[ ] per-client queue/backpressure bounded
[ ] status code explicitly tested
[ ] item type preserved through routers
[ ] SSE framing/reconnect behavior tested
[ ] client disconnect cleanup idempotent
```

## 43.33 `APIRouter` construction pattern

```python
router = APIRouter(
    prefix="/items",
    tags=["items"],
    dependencies=[Depends(require_user)],
    responses={404: {"description": "Not found"}},
)
```

Common responsibilities:

```text
prefix
shared tags
shared dependencies
shared response docs
custom route class
router-local path operations
nested routers
frontend surface
```

## 43.34 `include_router()` pattern

```python
app.include_router(
    router,
    prefix="/api/v1",
    tags=["public"],
    dependencies=[Depends(api_policy)],
)
```

Current 0.137+ model:

```text
include_router preserves original router/route objects in a live tree.
```

Do not mutate `router.routes` directly.

## 43.35 Router vs mount vs frontend

| Primitive | Composition semantics | OpenAPI |
|---|---|---|
| `include_router()` | same FastAPI API; metadata/dependencies compose | integrated |
| `app.mount()` | separate ASGI subapplication | separate/subapp-owned |
| `app.frontend()` | low-priority built frontend/static routing | not ordinary API operations |
| `StaticFiles` mount | generic static subapp | not ordinary API operations |

## 43.36 Current `app.frontend()` decision card

```python
app.frontend(
    "/",
    directory="dist",
    fallback="auto",
    check_dir="auto",
)
```

Current semantics to remember:

```text
API path operations take priority.
SPA fallback targets HTML navigation.
missing .js/.css/images remain 404.
non-GET/HEAD fallback misses remain 404.
check_dir="auto" cooperates with development environment.
frontend dependencies supported.
0.141.1 fixes dependency headers/background tasks.
```

## 43.37 Middleware quick matrix

| Middleware | Import | Main purpose |
|---|---|---|
| CORS | `fastapi.middleware.cors.CORSMiddleware` | browser cross-origin policy |
| Trusted Host | `fastapi.middleware.trustedhost.TrustedHostMiddleware` | host-header allowlist |
| HTTPS redirect | `fastapi.middleware.httpsredirect.HTTPSRedirectMiddleware` | force HTTP->HTTPS/WSS semantics |
| GZip | `fastapi.middleware.gzip.GZipMiddleware` | compress eligible responses |
| Session | `starlette.middleware.sessions.SessionMiddleware` | signed cookie sessions |
| custom function | `@app.middleware("http")` | request/response wrapper |
| custom ASGI | normal ASGI middleware class | HTTP + possibly WebSocket/protocol-wide behavior |

## 43.38 CORS secure-default card

```python
app.add_middleware(
    CORSMiddleware,
    allow_origins=["https://app.example.com"],
    allow_credentials=True,
    allow_methods=["GET", "POST"],
    allow_headers=["Authorization", "Content-Type"],
)
```

Rule:

```text
credentials -> explicit origins, not wildcard mental model
```

CORS is not authentication.

## 43.39 Error handling matrix

| Failure | FastAPI primitive | Typical status |
|---|---|---|
| explicit not found/conflict/auth etc. | raise `HTTPException` | chosen 4xx/5xx |
| request validation | `RequestValidationError` | 422 default |
| WebSocket policy | `WebSocketException` | WebSocket close code |
| domain exception | custom `@app.exception_handler` | mapped |
| unexpected exception | server error handler/logging | 500 |

Canonical:

```python
@app.exception_handler(DomainError)
async def domain_error(request: Request, exc: DomainError):
    return JSONResponse(
        status_code=409,
        content={"code": exc.code},
    )
```

## 43.40 Security primitive categories

FastAPI `fastapi.security` provides dependables for common OpenAPI security schemes, including categories such as:

```text
API key:
  APIKeyQuery
  APIKeyHeader
  APIKeyCookie

HTTP auth:
  HTTPBasic
  HTTPBearer
  HTTPDigest / related supported HTTP schemes as exposed by current reference

OAuth2:
  OAuth2PasswordBearer
  OAuth2AuthorizationCodeBearer
  base OAuth2 configuration

OpenID Connect:
  OpenIdConnect

OAuth form helpers:
  OAuth2PasswordRequestForm
  OAuth2PasswordRequestFormStrict

scope context:
  SecurityScopes
```

Always consult current `reference/security/` for the exact constructor surface of a chosen scheme.

## 43.41 Authentication dependency pattern

```python
oauth2 = OAuth2PasswordBearer(tokenUrl="/token")

async def get_current_user(
    token: Annotated[str, Depends(oauth2)],
) -> User:
    claims = verify_access_token(token)
    return await load_principal(claims)
```

Scope-aware:

```python
async def get_current_user(
    security_scopes: SecurityScopes,
    token: Annotated[str, Depends(oauth2)],
):
    ... verify token ...
    ... enforce declared scopes ...
```

Route:

```python
current_user: Annotated[
    User,
    Security(get_current_user, scopes=["items:read"]),
]
```

Then perform resource/tenant policy separately.

## 43.42 Auth transport decision matrix

| Client | Common architecture |
|---|---|
| same-site browser app | secure HttpOnly session cookie + CSRF design |
| SPA across domains | OIDC/OAuth tokens or BFF/session architecture |
| native/mobile | authorization code + PKCE |
| service-to-service | service identity/client credentials/mTLS/API key according to environment |
| simple internal script | API key/Bearer token if policy allows |

FastAPI exposes primitives; identity provider/session lifecycle design is application architecture.

## 43.43 OpenAPI endpoints matrix

| Purpose | Default |
|---|---|
| schema | `/openapi.json` |
| Swagger UI | `/docs` |
| ReDoc | `/redoc` |
| Swagger OAuth redirect | `/docs/oauth2-redirect` |

Disable:

```python
app = FastAPI(
    openapi_url=None,
    docs_url=None,
    redoc_url=None,
)
```

This changes discoverability, not authorization.

## 43.44 OpenAPI contract stability matrix

| Change | Client risk |
|---|---|
| remove operation/path | breaking |
| change method | breaking |
| rename required field | breaking |
| optional -> required input | breaking |
| widen response object with optional field | usually additive but test clients |
| enum adds value | can break exhaustive generated clients |
| operationId change | generated SDK method rename/break |
| status/content type change | often breaking |
| auth scheme/scopes change | security/client breaking |

Use semantic schema diff in CI for public APIs.

## 43.45 Operation ID policy

Manual:

```python
@app.get("/items/{id}", operation_id="items_get")
```

Global:

```python
def generate_id(route: APIRoute) -> str:
    ...

app = FastAPI(generate_unique_id_function=generate_id)
```

Operation IDs are part of generated-client compatibility.

## 43.46 Testing decision matrix

| Need | Tool/pattern |
|---|---|
| normal endpoint test | `TestClient(app)` |
| lifespan needed | `with TestClient(app) as client:` |
| async DB/client direct calls in test | `pytest.mark.anyio` + HTTPX `AsyncClient`/`ASGITransport` |
| async test + app lifespan | explicit lifespan manager |
| replace auth/external dependency | `app.dependency_overrides` |
| WebSocket | `client.websocket_connect()` |
| OpenAPI | `app.openapi()` or GET `/openapi.json` |
| DB isolation | override session dependency/test transaction |

## 43.47 `TestClient` skeleton

```python
from fastapi.testclient import TestClient


def test_endpoint():
    with TestClient(app) as client:
        response = client.get("/items/1")
        assert response.status_code == 200
        assert response.json()["id"] == 1
```

## 43.48 Async test skeleton

```python
import pytest
from httpx import ASGITransport, AsyncClient

@pytest.mark.anyio
async def test_async_endpoint():
    transport = ASGITransport(app=app)
    async with AsyncClient(
        transport=transport,
        base_url="http://test",
    ) as client:
        response = await client.get("/")
    assert response.status_code == 200
```

Remember: AsyncClient/ASGITransport does not automatically run FastAPI lifespan.

## 43.49 Dependency override skeleton

```python
app.dependency_overrides[get_current_user] = override_user
try:
    with TestClient(app) as client:
        ...
finally:
    app.dependency_overrides.clear()
```

Prefer fixture-owned setup/teardown.

## 43.50 CLI lookup matrix

| Goal | Command |
|---|---|
| dev auto-discovery | `fastapi dev` |
| dev specific file | `fastapi dev app/main.py` |
| dev explicit entrypoint | `fastapi dev --entrypoint app.main:app` |
| production-style | `fastapi run` |
| bind | `fastapi run --host 0.0.0.0 --port 8000` |
| multi-worker | `fastapi run --workers 4` |
| behind trusted proxy | add proxy-header configuration |
| stripped prefix | add `--root-path /prefix` |
| direct Uvicorn | `uvicorn app.main:app --host 0.0.0.0 --port 8000` |

Recommended stable project config:

```toml
[tool.fastapi]
entrypoint = "app.main:app"
```

## 43.51 Environment matrix

```text
local dev:
  FASTAPI_ENV=development
  fastapi dev

production:
  FASTAPI_ENV=production  # when application/current frontend behavior relies on it
  fastapi run
```

Do not use `FASTAPI_ENV` as a substitute for typed Settings.

## 43.52 Deployment topology matrix

| Environment | Process pattern |
|---|---|
| developer laptop | one `fastapi dev` process + reload |
| single VM | service manager + `fastapi run --workers N` if useful |
| Docker Compose single host | one/multiple workers per container according to topology |
| Kubernetes/ECS cluster | usually one app process/container, replicas at orchestrator |
| large model/GPU | often one process per GPU/model allocation; external inference possible |
| serverless | platform-specific ASGI adapter/startup constraints; minimize heavy cold start |

## 43.53 Worker multiplication matrix

If:

```text
workers = W
DB pool per worker = D
HTTP pool per worker = H
model memory per worker = M
```

Then approximate process-local capacity:

```text
DB connection capacity ~= W * D
HTTP pool capacity     ~= W * H
model memory           ~= W * M
```

Add interpreter/runtime/application overhead separately.

## 43.54 Proxy/root-path matrix

| Situation | Required thought |
|---|---|
| TLS terminated before Uvicorn | trusted forwarded scheme/host |
| public path prefix stripped by proxy | `root_path` |
| OpenAPI wrong server URL | inspect `root_path`, `servers`, proxy headers |
| redirect goes to internal HTTP | proxy scheme/host trust incorrect |
| OAuth callback wrong | public host/scheme/root path configuration |
| SSE seems buffered | proxy buffering/timeouts/compression |

## 43.55 Health probe matrix

| Probe | Question | Should depend on DB? |
|---|---|---|
| liveness | is process fundamentally alive? | usually no |
| readiness | should receive traffic now? | maybe critical dependencies, bounded check |
| startup | has slow initialization completed? | yes insofar as required startup work |

Avoid restart storms by making liveness too deep.

## 43.56 Database lifetime matrix

| Object | Lifetime |
|---|---|
| engine/pool | app/worker lifespan |
| ORM session | request/function dependency |
| transaction | business operation/UoW |
| connection | pool-managed, scoped as required |
| migration | one deployment pre-step, not each worker |

## 43.57 Database endpoint skeleton

```python
SessionDep = Annotated[Session, Depends(get_session)]

@app.get("/items/{item_id}", response_model=ItemPublic)
def get_item(item_id: int, session: SessionDep):
    item = session.get(Item, item_id)
    if item is None:
        raise HTTPException(404, "Item not found")
    return item
```

For async DB stack, use async session/driver end to end.

## 43.58 Settings skeleton

```python
from functools import lru_cache
from pydantic_settings import BaseSettings, SettingsConfigDict

class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", extra="ignore")

    app_name: str = "Example API"
    database_url: str
    debug: bool = False

@lru_cache

def get_settings() -> Settings:
    return Settings()
```

Test by overriding/passing settings, not mutating global environment repeatedly inside request tests.

## 43.59 App factory skeleton

```python
def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or get_settings()

    app = FastAPI(
        title=settings.app_name,
        lifespan=create_lifespan(settings),
    )

    install_middleware(app, settings)
    install_exception_handlers(app)
    install_routes(app)

    return app

app = create_app()
```

## 43.60 Large application package skeleton

```text
app/
  main.py
  settings.py
  lifespan.py
  api/
    dependencies.py
    errors.py
    routers/
      users.py
      orders.py
      admin.py
  domain/
    users/
    orders/
  infrastructure/
    db/
    http/
    messaging/
  observability/
  tests/
```

Direction rule:

```text
HTTP adapter -> application/domain -> infrastructure abstractions
composition root -> concrete implementations
```

## 43.61 WebSocket skeleton

```python
@app.websocket("/ws/{channel}")
async def ws(
    websocket: WebSocket,
    channel: str,
    user: Annotated[User, Depends(get_ws_user)],
):
    await websocket.accept()
    try:
        while True:
            payload = await websocket.receive_json()
            event = EventAdapter.validate_python(payload)
            await handle_event(user, channel, event)
    except WebSocketDisconnect:
        await cleanup_connection(websocket)
```

Multi-worker broadcast requires shared broker/pubsub.

## 43.62 Background work decision matrix

| Work | Primitive |
|---|---|
| tiny best-effort post-response notification | `BackgroundTasks` |
| must survive restart | durable queue/worker |
| minutes/hours | durable job API |
| CPU-heavy | process/compute worker |
| periodic singleton | scheduler/worker platform, not each API worker blindly |

## 43.63 Security regression matrix

```text
Transport:
  wrong/missing Content-Type
  oversized body
  invalid multipart

Identity:
  no token
  malformed token
  expired token
  wrong issuer/audience
  wrong token type

Authorization:
  insufficient scope
  wrong owner
  cross-tenant access
  disabled principal

Browser/proxy:
  disallowed CORS origin
  CSRF attempt
  spoofed Host
  spoofed forwarded headers

Input sinks:
  SQL injection payload
  path traversal filename
  SSRF URL
  unsafe command/template payload
```

## 43.64 Observability field card

Request completion event fields:

```text
request_id
trace_id
method
route_template
status_code
duration_ms
response_bytes
principal_type/tenant_class only if safe + bounded
error_code if applicable
```

Do not metric-label with user IDs/order IDs/raw paths/request IDs.

## 43.65 Performance triage order

```text
1. measure downstream DB/network wait
2. inspect event-loop blocking/threadpool wait
3. inspect N+1/query count
4. inspect pool contention
5. inspect serialization/validation
6. inspect application CPU
7. inspect framework overhead
8. tune worker count only with full resource model
```

Do not begin with micro-optimizing route decorators.

## 43.66 Current 0.140 large-app optimization card

```text
0.140.0  reduce dependency memory
0.140.1  dependency LRU cache sizing for large apps
0.140.2  stop retaining flat dep trees
0.140.3  avoid repeated OpenAPI dep flattening
0.140.4  skip unused repeat bookkeeping
0.140.5  avoid body-field dep flattening
0.140.6  avoid request-param dep flattening
0.140.7  avoid OpenAPI dep flattening
```

Migration implication:

```text
retest old workarounds / startup memory / OpenAPI generation on 0.141.1
```

## 43.67 Current streaming patch card

```text
0.140.8  preserve stream item type through include_router
0.140.11 respect response_model_* for Iterable non-generator endpoints
0.140.12 SSE line splitting spec fix
0.140.13 respect explicit status_code for SSE/JSONL
```

If a service relies heavily on streams, include these exact regressions in tests.

## 43.68 Current router migration card

From pre-0.137, search for:

```bash
rg '\.routes\b|APIRoute|APIRouter' app tests
```

Inspect:

```text
direct router.routes iteration
route mutation
custom flattening
custom OpenAPI traversal
route copying assumptions
```

Current rule:

```text
router inclusion = live tree; router.routes = internal detail
```

Advanced current helper:

```text
iter_route_contexts()
```

for cases formerly requiring direct route-tree assumptions.

## 43.69 Current frontend release card

```text
0.138.0 app.frontend/router.frontend
0.138.2 non-GET/HEAD fallback -> 404
0.139.0 frontend dependencies
0.141.0 check_dir="auto"
0.141.1 dependency headers/background tasks fix + FASTAPI_ENV docs
```

## 43.70 Version-upgrade test battery

```text
[ ] mypy/pyright/ty checks
[ ] unit tests
[ ] endpoint contract tests
[ ] request-validation tests
[ ] dependency teardown tests
[ ] auth/scope/tenant tests
[ ] OpenAPI semantic diff
[ ] generated SDK smoke test
[ ] router inclusion/custom route tests
[ ] JSONL/SSE/WebSocket tests
[ ] lifespan startup/shutdown tests
[ ] database transaction/session tests
[ ] large app startup/RSS/OpenAPI benchmark
[ ] load test p50/p95/p99 + error rate
[ ] canary metrics after deploy
```

## 43.71 FastAPI / Starlette / Pydantic ownership matrix

| Feature | Primary layer |
|---|---|
| ASGI app protocol | Starlette/ASGI |
| routing foundation | Starlette + FastAPI extensions |
| `Request`, `Response`, WebSocket | Starlette primitives re-exported/integrated by FastAPI |
| middleware implementations | mostly Starlette, re-exported conveniently by FastAPI |
| parameter/body validation | FastAPI orchestration + Pydantic |
| JSON Schema/model validation | Pydantic |
| dependency injection | FastAPI |
| OpenAPI operation construction | FastAPI |
| security scheme integration | FastAPI + OpenAPI models |
| server process | Uvicorn/other ASGI server |
| TestClient foundation | Starlette/HTTPX, re-exported by FastAPI |

Debugging rule: locate the owning layer before searching for a fix.

## 43.72 Common imports cheat sheet

```python
from typing import Annotated

from fastapi import (
    APIRouter,
    BackgroundTasks,
    Body,
    Cookie,
    Depends,
    FastAPI,
    File,
    Form,
    Header,
    HTTPException,
    Path,
    Query,
    Request,
    Response,
    Security,
    UploadFile,
    WebSocket,
    WebSocketDisconnect,
    WebSocketException,
    status,
)
from fastapi.exceptions import RequestValidationError
from fastapi.responses import (
    FileResponse,
    HTMLResponse,
    JSONResponse,
    PlainTextResponse,
    RedirectResponse,
    StreamingResponse,
)
from fastapi.routing import APIRoute
from fastapi.security import (
    APIKeyHeader,
    HTTPBearer,
    OAuth2AuthorizationCodeBearer,
    OAuth2PasswordBearer,
    SecurityScopes,
)
from fastapi.sse import EventSourceResponse, ServerSentEvent
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from fastapi.testclient import TestClient
```

Only import what the module owns; do not make every router depend on the whole framework surface.

## 43.73 Status constant examples

```python
from fastapi import status

status.HTTP_200_OK
status.HTTP_201_CREATED
status.HTTP_202_ACCEPTED
status.HTTP_204_NO_CONTENT
status.HTTP_400_BAD_REQUEST
status.HTTP_401_UNAUTHORIZED
status.HTTP_403_FORBIDDEN
status.HTTP_404_NOT_FOUND
status.HTTP_409_CONFLICT
status.HTTP_422_UNPROCESSABLE_CONTENT
status.HTTP_429_TOO_MANY_REQUESTS
status.HTTP_500_INTERNAL_SERVER_ERROR
status.WS_1008_POLICY_VIOLATION
```

Use named constants when they improve readability; numeric literals remain valid.

## 43.74 Request/response body semantics card

```text
GET body: protocol technically possible but generally avoid for interoperable API design.
POST/PUT/PATCH: body common.
204: do not send semantic response body.
HEAD: response body suppressed by HTTP semantics.
multipart: cannot simultaneously be ordinary application/json body.
Response returned directly: normal response_model body processing bypassed.
```

## 43.75 Validation semantics card

```text
caller sends strings in query/path/header/cookie
 -> FastAPI/Pydantic converts/validates to annotated Python types

JSON object
 -> Pydantic model validation

invalid request
 -> RequestValidationError
 -> 422 default
```

Do not rely on validation alone for authorization or sink-specific security.

## 43.76 Model design card

Prefer distinct models when semantics differ:

```text
UserCreate
UserUpdate
UserPublic
UserInternal / persistence model
```

Partial update:

```python
changes = payload.model_dump(exclude_unset=True)
```

Sensitive fields should be absent from public output schema, not merely "usually not populated".

## 43.77 API versioning card

Possible:

```text
path:       /api/v1
host:       v1.api.example.com
media/header: custom routing/version negotiation
```

Default for broad interoperability: path versioning when a major compatibility split is truly required.

Prefer additive evolution and deprecation over gratuitous new versions.

## 43.78 Generated-client card

Before generating clients, stabilize:

```text
operationId
schema names
requiredness/nullability
response statuses
security schemes/scopes
route tags
```

Run client generation/smoke tests in CI when SDKs are a product artifact.

## 43.79 Production logging card

```text
stdout/stderr structured logs
request ID / trace ID
route template
status
latency
stable error code
no auth token/cookie/password/body by default
```

Keep detailed traceback server-side for unexpected errors.

## 43.80 Deployment pre-step card

Run **once** before replicas/workers when needed:

```text
DB migrations
schema registry changes
one-time data migration
asset build/publication if part of release pipeline
```

Do not let every worker race the same migration.

## 43.81 Container card

```dockerfile
FROM python:3.14
WORKDIR /code
# install pinned dependencies first
# copy app second
CMD ["fastapi", "run", "app/main.py", "--port", "80"]
```

Production refinements:

```text
non-root user
lockfile/frozen install
no secrets in image
read-only filesystem where possible
health probes
graceful termination
one process/container in cluster by default
```

## 43.82 Reverse-proxy card

```text
public HTTPS -> trusted proxy -> internal HTTP FastAPI
```

Verify:

```text
forwarded scheme
forwarded host
client IP trust
root_path
redirect Location
Swagger server URL
OAuth callback URL
stream buffering
```

## 43.83 Anti-pattern quick index

### Declaration

```text
untyped dict bodies everywhere
manual JSON schema duplicating Pydantic
unstable operation IDs for generated clients
huge response_model include/exclude lists instead of public model
```

### Async

```text
time.sleep() inside async def
blocking DB/client in async def
CPU-heavy loop on event loop
```

### DI

```text
Depends(dependency())
use_cache=False everywhere
business layer coupled to Request/HTTPException
one giant dependency doing auth+DB+business+serialization
```

### Lifetimes

```text
pool/client created per request
request session stored globally
must-not-lose job in BackgroundTasks
open DB transaction across slow stream unnecessarily
```

### Routing

```text
5,000-line main.py
router.routes mutation
mount() used merely for modular code
request-time route mutation
```

### Security

```text
CORS as auth
wildcard assumptions with credentials
untrusted forwarded headers
JWT signature-only validation
client header chooses tenant authoritatively
raw upload filename -> filesystem path
caller URL -> internal HTTP fetch without SSRF policy
```

### Deployment

```text
fastapi dev in prod
reload in prod
N workers x N replicas without resource model
DB pool sizing per app rather than per process
migration in every worker
```

### Testing

```text
no lifespan management when needed
dependency overrides leak across tests
only happy-path tests
no OpenAPI/stream/auth contract tests
```

## 43.84 Agent preflight — creating a new endpoint

```text
[ ] Choose HTTP method/path and stable operation identity.
[ ] Declare source of every input: path/query/header/cookie/body/form/file/dependency.
[ ] Use Pydantic model for structured input.
[ ] Add constraints at schema boundary.
[ ] Resolve principal/tenant/resource policy through dependencies/services.
[ ] Define output model and success status.
[ ] Document expected non-success responses.
[ ] Choose normal JSON vs JSONL vs SSE vs WebSocket vs file/stream.
[ ] Ensure resource lifetimes match response lifetime.
[ ] Add invalid-input/auth/resource tests.
[ ] Verify OpenAPI output.
```

## 43.85 Agent preflight — creating a new dependency

```text
[ ] Is this actually request/runtime infrastructure or should it be a normal function?
[ ] What inputs/subdependencies does it require?
[ ] Is default per-request caching correct?
[ ] Does it allocate a resource requiring yield cleanup?
[ ] Should cleanup be function or request scope?
[ ] Can a stream outlive the resource?
[ ] Does the dependency expose OAuth scopes? Use Security if needed.
[ ] Can tests override it cleanly?
[ ] Is sensitive data excluded from logs/schema?
```

## 43.86 Agent preflight — application startup

```text
[ ] Settings validated before serving.
[ ] Long-lived HTTP/DB clients created once per worker.
[ ] Lifespan cleanup closes all clients/pools.
[ ] No migration race across workers.
[ ] Frontend build directory policy correct for env.
[ ] OpenAPI generation succeeds.
[ ] Readiness stays false until required startup complete.
[ ] Startup time/memory measured if large app/model.
```

## 43.87 Agent preflight — production deployment

```text
[ ] exact FastAPI version + lockfile
[ ] Python runtime version supported
[ ] fastapi run / Uvicorn, not dev/reload
[ ] process count decided by topology
[ ] per-worker memory + DB pool calculated
[ ] external supervisor/orchestrator restart policy
[ ] TLS termination + trusted proxy headers
[ ] root_path correct if prefix stripped
[ ] CORS/Host policy correct
[ ] request/body/upload limits
[ ] downstream timeouts
[ ] health probes
[ ] graceful termination budget
[ ] structured logs + metrics + traces
[ ] secrets injected, not baked/logged
```

## 43.88 Agent preflight — version upgrade

```text
[ ] read every skipped FastAPI release note
[ ] inspect Starlette/Pydantic changes if relevant
[ ] update uv.lock in isolated branch
[ ] run type checks
[ ] run full tests
[ ] semantic OpenAPI diff
[ ] generated client smoke test
[ ] route tree/custom APIRoute tests
[ ] dependency yield teardown tests
[ ] JSONL/SSE status/framing tests
[ ] startup/OpenAPI RSS/time benchmark for large app
[ ] load test critical paths
[ ] canary with error/latency/memory monitoring
```

## 43.89 FastAPI 0.141.1 source-of-truth hierarchy

For LLM/programming-agent work, use this hierarchy:

```text
1. FastAPI current API reference
   https://fastapi.tiangolo.com/reference/

2. FastAPI current tutorial / advanced / how-to docs
   https://fastapi.tiangolo.com/

3. FastAPI release notes
   https://fastapi.tiangolo.com/release-notes/

4. FastAPI package metadata
   https://pypi.org/project/fastapi/

5. Starlette docs for underlying ASGI primitives
   https://www.starlette.io/

6. Pydantic v2 docs for validation/schema/settings details
   https://docs.pydantic.dev/latest/

7. Uvicorn docs for server/process/proxy settings
   https://www.uvicorn.org/
```

When exact call signatures matter, prefer the current reference page/source corresponding to the pinned version over older blog/tutorial code.

## 43.90 Primary FastAPI page index by task

| Task | Primary page |
|---|---|
| app constructor | `https://fastapi.tiangolo.com/reference/fastapi/` |
| router | `https://fastapi.tiangolo.com/reference/apirouter/` |
| APIRoute | `https://fastapi.tiangolo.com/reference/apiroute/` |
| parameters | `https://fastapi.tiangolo.com/reference/parameters/` |
| dependencies | `https://fastapi.tiangolo.com/reference/dependencies/` |
| request | `https://fastapi.tiangolo.com/reference/request/` |
| responses | `https://fastapi.tiangolo.com/reference/responses/` |
| security | `https://fastapi.tiangolo.com/reference/security/` |
| WebSocket | `https://fastapi.tiangolo.com/reference/websockets/` |
| SSE | `https://fastapi.tiangolo.com/reference/sse/` |
| testing | `https://fastapi.tiangolo.com/reference/testclient/` |
| OpenAPI | `https://fastapi.tiangolo.com/reference/openapi/` |
| encoders | `https://fastapi.tiangolo.com/reference/encoders/` |
| static files | `https://fastapi.tiangolo.com/reference/staticfiles/` |
| templates | `https://fastapi.tiangolo.com/reference/templating/` |
| CLI | `https://fastapi.tiangolo.com/fastapi-cli/` |
| release changes | `https://fastapi.tiangolo.com/release-notes/` |

## 43.91 Final LLM-agent invariants

**Invariant 1 — Python signatures are protocol contracts.**

FastAPI uses annotations/defaults/markers to decide request extraction, validation, DI, output, and OpenAPI. Treat them as API definitions, not cosmetic typing.

**Invariant 2 — Response models are security/contract boundaries.**

Returning an ORM/internal object does not mean all its fields should become public. Keep public output models explicit.

**Invariant 3 — `Response` is an escape hatch.**

Returning a Response directly gives exact control and bypasses normal output model conversion for the body. Use intentionally.

**Invariant 4 — DI owns request-scoped infrastructure, not all application architecture.**

Keep domain/service functions normally callable and use FastAPI dependencies at the adapter/runtime boundary.

**Invariant 5 — lifetimes must match use.**

App resources -> lifespan. Request resources -> dependencies. Durable state/work -> external systems.

**Invariant 6 — async means cooperative I/O, not CPU parallelism.**

Never put blocking or CPU-heavy work on the event loop by changing `def` to `async def` mechanically.

**Invariant 7 — worker memory/state is process-local.**

Every worker has its own globals, pools, app lifespan, cache, and model allocation.

**Invariant 8 — current router inclusion is live-tree composition.**

Since 0.137, routers/routes are preserved rather than cloned; `router.routes` is internal detail.

**Invariant 9 — current FastAPI has first-class streams.**

Use JSON Lines, SSE, raw streaming, or WebSockets according to semantics rather than forcing every long result into ordinary JSON.

**Invariant 10 — `app.frontend()` is a current first-class surface.**

Use it for built SPA/static frontend integration when appropriate; keep StaticFiles/templates/mount semantics distinct.

**Invariant 11 — OpenAPI is a product contract.**

Operation IDs, model schemas, statuses, media types, and security schemes can be client-breaking changes.

**Invariant 12 — security requires application policy.**

Validation/CORS/OAuth declarations do not replace resource authorization, tenant isolation, trusted proxy policy, size limits, SSRF/file safety, secrets, and audit controls.

**Invariant 13 — production behavior is the whole stack.**

FastAPI + Starlette + Pydantic + Uvicorn + proxy + DB/client pools + process manager/orchestrator all materially affect correctness and performance.

**Invariant 14 — upgrade from evidence.**

Pin, read release notes, run tests, diff OpenAPI, benchmark the relevant large-app/streaming paths, and canary.

---

# End of FastAPI 0.141.1 advanced reference

Reference target: **FastAPI 0.141.1** — stable release dated **2026-07-29**.

For deployment code generated from this document, verify exact API signatures against the pinned FastAPI reference and resolved lockfile before rollout, especially after any dependency upgrade.
