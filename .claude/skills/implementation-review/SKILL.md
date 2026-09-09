---
name: implementation-review
description: Review a requested plan or implementation for behavior, material risk, and missing scope without changing production code.
---

# Independent review

Read the requested scope and current target. Inspect relevant code, consumers, and
attributable checks. Distinguish observed defects from unverified concerns.
Prioritize findings by user impact, include concrete locations and correction options,
and state validation limits. A missing ceremonial field or named oracle is not a defect.
For plan review, challenge real dependencies, omitted consumers, and untestable outcomes.
For code review, trace actual behavior and meaningful failure paths. Keep useful tests;
request new ones only where they resolve material uncertainty.
Write the requested Markdown review under `docs/reviews/`. Ordinary reviews need no
artifact-type registration or approval cycle. Do not implement fixes unless requested.
