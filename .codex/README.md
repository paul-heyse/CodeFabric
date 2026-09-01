# Codex configuration for CodeFabric

This project layer gives Codex the same repository context as Claude Code while making
fresh non-interactive commands deterministic. The environment policy, execpolicy rules,
and SessionStart hook were verified with **codex-cli 0.151.0** on 2026-08-29. The
approval/sandbox posture described below replaced the verified permission profile on
2026-08-30 and has not itself been re-verified.

```text
.codex/
  config.toml              approval/sandbox posture, proxy allowances, environment
                           policy, and SessionStart hook
  hooks/session-start.sh   emits the read-only bootstrap report
  rules/codefabric.rules   narrow UDS-test exceptions (inert under the current posture)
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

## Approval and sandbox posture

`config.toml` sets the posture for this project directly, taking precedence over the user
layer:

| Setting | Value | Effect |
|---|---|---|
| `approval_policy` | `never` | commands run without approval prompts |
| `sandbox_mode` | `danger-full-access` | no filesystem or network confinement |
| `web_search` | `live` | live web search available to the session |

`[features.network_proxy]` matches that posture: local binding, upstream proxying,
non-loopback proxying, SOCKS5 over TCP and UDP, `"*" = "allow"` for domains, and
`dangerously_allow_all_unix_sockets = true`.

This replaced the `codefabric` permission profile on 2026-08-30. That profile extended
Codex's workspace profile with read-only access to the dedicated sccache cache directory
and to the two cache sockets, and was selected by `default_permissions`; the profile, its
filesystem grants, and the selector are all gone.

The two sccache sockets remain listed under `[features.network_proxy.unix_sockets]`:

- `/private/tmp/codefabric-sccache/server.sock` on macOS;
- `/tmp/codefabric-sccache/server.sock` on Linux.

Under `dangerously_allow_all_unix_sockets` they grant nothing extra. They are kept as the
record of the only socket a build genuinely needs, so a narrower posture can be restored
without rediscovering it.

The cache trust split is unchanged, because it is a property of the cache design rather
than the sandbox: the supervised host service owns cache mutation, and sccache client-side
mode performs reads in the compiler process.

## UDS-dependent test rules

**Currently inert.** With `sandbox_mode = "danger-full-access"` nothing is confined, so no
command needs an escape rule. The rules are retained because they encode the correct narrow
answer if the sandbox is ever restored, and the paragraphs below describe them in that
sandboxed context.

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

Project-local configuration, rules, and hooks load only for a trusted project, keyed by the
clone's absolute path — `/home/paul/CodeFabric` on Linux, `/Users/paulheyse/CodeFabric` on
macOS. Collaborators must trust their own clone.

Model and reasoning choices remain user preferences; approval policy and sandbox mode do
not — `config.toml` sets both for this project. No external MCP server is registered here:
the repository contains the adapter implementation, while host registration and the
production serving surface remain owned by their serving waves.
