# CodeFabric CPG MCP adapter

This project is the presentation-only FastMCP 4 STDIO adapter between an MCP host and the
CodeFabric daemon v2 contract. It exposes the modern protocol-era tools and public resources,
while all source state, Arrow/DataFusion execution, Delta state, authorization, and durable
query lifecycle remain in the Rust daemon.

Production launch is mediated by the attach-only Rust launcher:

```text
codefabric mcp serve --supervisor <discovery-path> --policy-id <opaque-policy-id>
```

The launcher attaches to the existing workspace supervisor, obtains one bounded launch grant,
starts the policy-selected installed adapter, and passes the launch envelope over inherited file
descriptor 3. Environment variables do not supply workspace, daemon, or capability authority.
Launching the Python module directly is therefore not a production route and fails closed without
that inherited socket.

Standard output is reserved exclusively for MCP protocol frames. The direct module's
`--identity` option writes its fixed runtime identity to standard error for installation checks.
The locked development and runtime interpreter is Python 3.14.7; `pyproject.toml` requires at
least that version.

Repository checks use the adapter-local environment:

```text
just adapter-ci-fast
just adapter-wheel-test
```
