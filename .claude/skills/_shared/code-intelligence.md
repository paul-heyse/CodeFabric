# Bounded code and documentation navigation

Use `rg --files` and `rg` to locate relevant sources; use semantic/structural tools where
they answer reference or syntax questions more precisely. A text search is not whole-program
proof; a compiler checks only its selected build/configuration. Escalate breadth when needed.
Use `just spec-outline` for current specifications and `just lib-outline` for library chapters.
Read exact relevant sections and API sources, not entire corpora. Preserve owned/provider and
language boundaries when tracing behavior. Never print `.envrc.local` or capability tokens.
