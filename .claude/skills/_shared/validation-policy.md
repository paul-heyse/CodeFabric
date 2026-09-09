# Selecting validation

Use the smallest checks that answer the changed behavior's risk. Docs need relevant
spelling/links; tooling needs focused tests; code needs affected checks and consumers;
product claims need real product scenarios. Invalidation and recovery need selected
history/failure cases. Use integrated suites at meaningful integration/release boundaries.
Run expensive fuzzing/mutation/profiling only for relevant uncertainty. Reuse existing tests.
Never turn known failures into passes with skips or altered expectations. Nextest excludes
doctests. Record failures and untested scope; do not run full CI before every edit.
