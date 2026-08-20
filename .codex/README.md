# Codex configuration for CodeFabric

Mirrors the Claude Code setup in `.claude/` so both agents open a session with
the same skills and the same environment report. Verified against
**codex-cli 0.148.0** on 2026-08-20.

```
.codex/
  config.toml              project config — skills note + SessionStart hook
  hooks/session-start.sh   emits the bootstrap environment report
  skills -> ../.claude/skills
  README.md                this file
.agents/
  skills -> ../.claude/skills
```

## Skills — one copy, two agents

Both symlinks point at `.claude/skills`, so a skill is edited once and both
agents see it. The alternative — real copies under `.codex/skills` — was
rejected: this repository has already spent real effort repairing citations that
drifted between duplicated documents.

Two symlinks exist because `.agents/skills` is the path OpenAI documents, while
`.codex/skills` is what 0.148.0 actually scans and matches the layout requested
here. Keeping both means a future Codex release can drop either without
breaking discovery.

**Verified.** `codex debug prompt-input` lists all 18 project skills. Each
symlink alone yields 18; both together still yield 18, so there is no
double-listing. `_shared/` is correctly skipped — it holds shared policy text
with no `SKILL.md`, so it is not a skill.

Re-check after a Codex upgrade:

```bash
codex debug prompt-input | python3 -c '
import sys, json, re
t = json.load(sys.stdin)[0]["content"][0]["text"]
names = re.findall(r"^- ([a-z0-9-]+):.*CodeFabric", t, re.M)
print(len(names), "project skills:", " ".join(names))'
# expect: 18
```

## The SessionStart hook

Runs `scripts/bootstrap.sh --context` and injects the result as developer
context — the same report the Claude hook in `.claude/settings.json` produces,
so a session starts knowing toolchain state instead of probing for it.

The script is safe by construction: any failure exits 0 with no output, so a
broken bootstrap degrades the report rather than the session. It resolves the
repo root from its own location, so it also works when Codex starts in a
subdirectory. It emits the documented
`hookSpecificOutput.additionalContext` JSON, falling back to plain stdout
(also accepted) if `python3` is unavailable.

Test it directly — this needs no Codex session and no model call:

```bash
./.codex/hooks/session-start.sh </dev/null
```

### What is verified, and what is not

**Verified:** the project config layer is live. Introducing a deliberate TOML
syntax error makes `codex debug prompt-input` exit 1; restoring it returns 0.
So `.codex/config.toml` is genuinely parsed, not silently skipped. The `hooks`
feature is `stable` and `true` in `codex features list`.

**Not verified: whether the hook actually fires in an interactive session.**
It could not be tested from this non-interactive harness, because
**`codex exec` does not run `SessionStart` hooks at all** — confirmed by
temporarily configuring the same hook in `~/.codex/config.toml`, where it also
did not fire. That rules out a repo-local-config fault as the explanation and
means `codex exec` is simply not a valid probe for this event.

Confirm it yourself in one interactive session:

```bash
codex          # look for the "Loading CodeFabric environment" status line,
               # then ask: "what did the session-start hook tell you?"
```

If it does not fire, the likely cause is
[openai/codex#17532](https://github.com/openai/codex/issues/17532) — repo-local
`.codex/config.toml` hooks not honored in interactive sessions, open as of
0.148.0. The documented fallback is a repo-local `.codex/hooks.json` with the
same handler:

```json
{ "hooks": { "SessionStart": [ { "matcher": "^startup$|^resume$|^clear$|^compact$",
  "hooks": [ { "type": "command", "command": ".codex/hooks/session-start.sh",
               "statusMessage": "Loading CodeFabric environment",
               "timeout": 20, "additionalContextLimit": 2500 } ] } ] } }
```

Only one of the two should exist at a time, or the report may be injected
twice.

## Trust

Codex ignores the project layer entirely for untrusted projects — a clone must
not be able to widen its own permissions by shipping a config file. This
repository is already trusted in `~/.codex/config.toml`:

```toml
[projects."/Users/paulheyse/CodeFabric"]
trust_level = "trusted"
```

A collaborator cloning the repo needs their own entry, or nothing in `.codex/`
takes effect.

## What is deliberately absent

**No sandbox, approval, or permissions block.** Those stay in your
`~/.codex/config.toml`. Claude's `.claude/settings.json` carries a ~90-entry
`Bash(...)` allowlist, which has no equivalent here: Codex governs by sandbox
mode plus approval policy, not per-command matching. Translating it would have
meant inventing a policy the repository has not agreed on. If you later want
the intent expressed in Codex's own model, the shape is:

```toml
sandbox_mode = "workspace-write"
approval_policy = "on-request"

[permissions.codefabric.filesystem]
":workspace_roots"."Cargo.lock"   = "deny"   # matches Claude's Edit() denies
":workspace_roots"."uv.lock"      = "deny"
":workspace_roots"."\.envrc\.local" = "deny" # holds CODEFABRIC_CPG_CAPABILITY_TOKEN
```

**No model or reasoning settings.** Those are your preference, not repository
policy, and already live in `~/.codex/config.toml`.

**No MCP servers.** CodeFabric's own FastMCP server is specified but not built
— it arrives in roadmap Wave 18.

**No `project_doc_fallback_filenames` override.** Codex reads `AGENTS.md` by
default, and `AGENTS.md` already routes to `CLAUDE.md` for the system-being-built
context and to `docs/spec_index/` for the specification map.
