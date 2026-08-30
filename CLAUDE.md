@AGENTS.md

# CLAUDE.md — Claude Code notes for CodeFabric

The line above imports `AGENTS.md`, which is the **canonical agent instruction set**: what
this repository is, the design corpus and its doctrine, the command contract, the assurance
tiers, the tooling inventory, and the invariants. Codex loads that file directly; this import
is how Claude Code loads the same content, so both agents read exactly the same rules. Do not
restate any of it here — a second copy is how the two drifted apart in the first place.

This file holds only what is specific to the Claude Code harness.

## Session start

A `SessionStart` hook in `.claude/settings.json` runs `scripts/bootstrap.sh --context` and
injects the result. That block already answers the repository specification's §59
session-bootstrap list — environment, toolchain, working-tree state, the cached gate baseline,
the everyday recipes, the traps, the search hazards, corpus sizes, and the dependency pins
extracted live from `FAB §2.1`.

**Do not re-derive what it already told you.** If it is in the context block, it is current as
of session start; re-run `./scripts/bootstrap.sh --context` only if you suspect the tree moved
underneath you.

Two commands extend it:

```bash
./scripts/bootstrap.sh --baseline   # run just ci-fast once and cache the tree-scoped verdict
./scripts/bootstrap.sh --quiet      # silent when healthy; use in scripts
```

The baseline is what §59.1 asks for. It is cached under `target/agent/`, so read the verdict
from the context block rather than re-running the gate; a red verdict there is a **pre-existing**
failure and must not be attributed to your change.

## Shell

Every Bash call is a fresh non-interactive shell that inherits nothing from the previous one,
and direnv's prompt hook never touches it. Repository recipes handle that boundary themselves:

```bash
just <recipe>
```

Do not source `scripts/bootstrap.sh` or invoke `direnv exec` for routine commands. The
bootstrap script only verifies/reports state. Just removes inherited Python, Conda, direnv,
and Rust overrides; selects the correct Rustup toolchain; and invokes uv with the absolute
adapter project and repository-local cache. There is no root uv project.

`direnv` needs `direnv allow` once and supplies interactive convenience only; root `.envrc`
does not sync or activate the adapter. Secrets — notably `CODEFABRIC_CPG_CAPABILITY_TOKEN`
— live in `.envrc.local`, which `.envrc` sources and `.gitignore` excludes. Widening a
search's ignore stack (`rg -uu`, `ast-grep --no-ignore`) reaches that file, so scope it to a
path rather than making it a default.

## Skills

`.claude/skills/` is the source of truth for all 21 skills; `.codex/skills` and `.agents/skills`
symlink to it so Codex reads the same copies. `AGENTS.md §10.2` describes them.

`_shared/` holds the policy the workflow skills load — it contains no `SKILL.md` and is
therefore not itself a skill.

## Toolchain

`rust-toolchain.toml` pins **stable** for the root daemon/data plane. The separate
`rustc-extractor/` root pins `nightly-2026-08-18` with `rustc-dev`, `rust-src`, and
`llvm-tools`; that is the extractor's production toolchain and never changes the root pin.
The dated nightly, exact compiler identity, golden corpus, and managed upgrade procedure are
the accepted boundary required by repo-spec §76 and the fact-generation design.
