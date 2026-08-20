# CodeFabric CPG MCP adapter

This project is the local, single-user STDIO adapter between an MCP host and
the CodeFabric daemon. Wave 0 establishes only the locked process, settings,
identity, lifecycle, and protocol shell; tools and daemon RPC integration land
in later implementation waves.

The host launches the adapter from the repository root with:

```text
uv run --frozen --project codefabric-cpg-mcp python -m codefabric_cpg_mcp
```

The process requires `CODEFABRIC_CPG_DAEMON_TARGET`,
`CODEFABRIC_WORKSPACE_ID`, and `CODEFABRIC_CPG_CAPABILITY_TOKEN`. Standard
output is reserved exclusively for MCP protocol frames. Runtime identity is
available on standard error with the `--identity` option.
