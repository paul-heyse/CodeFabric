# Native dependency checks

The dedicated allocator-receipt harnesses were retired with their APIs and local
forks on 2026-09-09. Native integration now uses the same upstream versions selected
by the application manifest and lock; see [source selection](../../third_party/native/README.md).

`just native-assurance-test` exercises the application's real runtime joins,
control headroom, shared pools, local-store ownership, exact Delta reads and native
maintenance boundaries. `just golden` exercises actual daemon publication/query,
cancellation and reopen. Use these alongside `just stable-graph-check` when changing
native dependencies. These are behavioral checks, not allocator/RSS certification.
