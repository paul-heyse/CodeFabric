# Codex configuration for CodeFabric

This project layer gives Codex the same repository context as Claude Code while making
fresh non-interactive commands deterministic. It was verified with **codex-cli 0.151.0**
on 2026-08-29.

```text
.codex/
  config.toml              environment policy, permission profile, and SessionStart hook
  hooks/session-start.sh   emits the read-only bootstrap report
  rules/codefabric.rules   narrow UDS-test exceptions
  skills -> ../.claude/skills
  README.md                this file
.agents/
  skills -> ../.claude/skills
```

## Skills — one copy, two agents

Both symlinks point at `.claude/skills`, so a skill is edited once and both agents read the
same copy. `.agents/skills` is the documented discovery path; `.codex/skills` preserves the
project's existing CLI discovery path. Codex deduplicates the two links.

## Clean command environment

`shell_environment_policy` inherits only Codex's core environment and excludes inherited
Python/Conda/direnv state, Rust toolchain and wrapper overrides, caller-owned Cargo target
directories, caller-owned incremental policy, and the adapter capability token. This is
defense in depth: every linewise
Just recipe independently establishes the same clean boundary with
`scripts/repo-shell.sh`, so the command contract also works outside Codex.

Use `just <recipe>` directly. Do not source `scripts/bootstrap.sh` or invoke `direnv exec`
for routine commands.

## Compiler-cache permission profile

The `codefabric` profile extends Codex's workspace profile and grants only the extra access
sccache needs locally:

- read access to the dedicated cache directory;
- access to `/private/tmp/codefabric-sccache/server.sock` on macOS or
  `/tmp/codefabric-sccache/server.sock` on Linux;
- the active network proxy required for enforcement of the Unix-socket allowlist.

It does not grant arbitrary cache writes or broad Unix-socket access. The supervised host
service owns cache mutation, while sccache client-side mode performs reads in the sandboxed
compiler process. This profile and the canary were exercised with `codex sandbox -P
codefabric`.

Permission profiles do not compose with legacy `sandbox_mode` settings. If a user or CLI
profile explicitly selects the legacy sandbox, that selection takes precedence over
`default_permissions`.

## UDS-dependent test rules

Adapter tests create randomly named Unix sockets, which cannot be safely represented by an
exact socket allowlist. `rules/codefabric.rules` therefore allows only these exact,
non-mutating recipe invocations to leave the sandbox:

```text
just adapter-test
just root-test
just ci-fast
just environment-regression
```

The rules deliberately do not allow bare `just`, recipe variants, `--justfile`, or mutating
recipes. Each rule carries positive and negative load-time examples. Verify decisions after
a Codex upgrade with:

```bash
codex execpolicy check --rules .codex/rules/codefabric.rules -- just adapter-test
codex execpolicy check --rules .codex/rules/codefabric.rules -- just proto-gen
```

The first decision must be `allow`; the second must have no matching rule.

## SessionStart hook

The inline `SessionStart` hook runs `.codex/hooks/session-start.sh`, which invokes the
read-only environment verifier and injects its stdout as additional context. It reports the
resolved Rust executables/toolchains, absolute uv project/cache, supervised sccache health
and cumulative lookup/non-cacheable telemetry, applicability of the narrow UDS rules, and
current tree/cached-baseline state. It does not activate a shell or persist environment
changes.

Test the handler directly:

```bash
./.codex/hooks/session-start.sh </dev/null
```

Project-local hooks require a trusted project and may require review after their definition
changes. Use `/hooks` in the Codex CLI to inspect and trust the hook if a session reports
that it was skipped.

## New Codex worktrees

Configure the ChatGPT desktop app's project-local environment setup script as:

```bash
just setup
```

That idempotently syncs the locked adapter environment and installs or refreshes the
supervised cache service for all current Git worktrees. The desktop app owns the generated
local-environment file under `.codex/`; do not guess its schema by hand. Once generated,
commit that file so other desktop users inherit the setup action.

## Trust and deliberate omissions

Project-local configuration, rules, and hooks load only for a trusted project. This machine
already trusts `/Users/paulheyse/CodeFabric`; collaborators must trust their own clone.

Model and reasoning choices remain user preferences. No external MCP server is registered
here: the repository contains the adapter implementation, while host registration and the
production serving surface remain owned by their serving waves.
