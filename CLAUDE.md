@AGENTS.md

# Claude harness notes

`AGENTS.md` is the canonical instruction source. Start from `STATUS.md`.
The SessionStart hook runs `scripts/bootstrap.sh --context` to report the environment;
it does not activate a plan, install tools, or run CI. A dated cached check is historical
evidence, not a requirement to rerun every gate before editing.

Skills are discovered from `.claude/skills`; Codex reads the same source through symlinks.
Use `just --list`, then affected checks. Python tools use the adapter dev environment.
Honor scoped documentation/preparation requests without changing production behavior.
