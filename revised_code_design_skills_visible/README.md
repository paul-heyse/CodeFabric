# Revised Code Design and Implementation Skills — Visible Bundle

This archive deliberately uses a visible top-level `skills/` directory rather
than a hidden `.claude/` directory, so every `SKILL.md` is visible in ordinary
file explorers and archive viewers.

## Contents

The ten main skills are located at:

- `skills/design-development/SKILL.md`
- `skills/library-capability-research/SKILL.md`
- `skills/impl-plan/SKILL.md`
- `skills/plan-audit/SKILL.md`
- `skills/integrate-plan-audit/SKILL.md`
- `skills/impl-plan-exec/SKILL.md`
- `skills/impl-status/SKILL.md`
- `skills/implementation-review/SKILL.md`
- `skills/lib-leverage/SKILL.md`
- `skills/skill-eval/SKILL.md`

Shared references used by the skills are under `skills/_shared/`.

## Installation

From the extracted bundle directory, copy the visible `skills/` directory into
your repository's `.claude/` directory:

```bash
mkdir -p /path/to/repository/.claude
cp -R skills /path/to/repository/.claude/
```

After copying, the installed layout will be:

```text
/path/to/repository/.claude/skills/<skill-name>/SKILL.md
```

An installation helper is included:

```bash
./install.sh /path/to/repository
```

It refuses to overwrite an existing skill directory unless `--force` is
provided.

## Why the prior archive looked empty

The prior archive did contain the skill files, but they were all below
`.claude/skills/`. Directories beginning with `.` are hidden by default in many
file explorers, including macOS Finder and many Linux file managers.
