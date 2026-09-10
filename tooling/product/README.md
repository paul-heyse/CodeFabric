# Product corpus and measurement

`just golden --list` lists existing real supervisor/daemon/installed-MCP scenarios. `just golden --case startup` selects one; no case runs in a mock product. An empty nextest selection, missing prerequisite, child failure, timeout or oversized output fails. Owned process groups are terminated and drained. Reports under `target/product/` identify selected/not-run cases and limitations. A selected-case pass is not complete-product certification.

`just golden --scenario /absolute/scenario.json --expect /absolute/expected.json` runs the existing modern STDIO client against an explicitly configured real supervisor and compares independently authored report fragments. The scenario uses the driver's current server/steps contract, including actual `codefabric mcp serve --supervisor <discovery> --policy-id <id>`. Never put launch secrets in fixtures. This mode does not fabricate workspace registration or bypass authorization. The Rust integration fixture provides current private registration/setup.

The [mixed-language fixture](../../tests/fixtures/pragmatic_cpg/README.md) provides independent source/semantic expectations and edit sequences. Its expectation document is a semantic specification, not a driver-report schema: wiring concrete public answers and source/context registration is coupled to production outcomes 4–6. Missing current runtime support must fail or return truthful remainder, never become an empty complete result.

`corpus.clean_incremental` applies bounded fixture edits and uses runtime-specific capture/rebuild/convergence callbacks. Its comparison preserves exact selected semantic identity, positions, order and coverage. The adapter must choose an explicit projection to remove only incidental values (for example a request nonce); it must retain identity relationships and all fields under test. Do not normalize missing providers to complete empty facts. Harness tests use controlled responses only to test comparison/failure behavior.

`just product-bench --case startup --samples 3` records repeated real scenario wall times, including build and harness costs, and references per-sample product reports. A failed sample produces no successful latency statistic. The existing compiled-release benchmark retains execution, process-tree sampling and statistical helpers without source freezes or proving-lineage requirements. The synthetic semantic-profile hashing workload remains a microbenchmark and is not CPG performance evidence.

| Prepared workload | Observable now | Production instrumentation needed |
|---|---|---|
| Startup and persisted reopen | Real integration scenario success/failure and total wall time | Daemon-only phase timings and retained bytes |
| Query/reference/source | Modern client request/response, errors and expected fragments | Stable phase timing, corpus public-answer adapters for both languages |
| Edit/add/delete/rename/context | Reusable source sequence and exact comparator | Relevant-scope convergence predicate and actual clean-rebuild adapter |
| Semantic failure/repair | Broken and repaired Rust source, missing Python import | Per-context/family failure and remainder assertions |
| Cancellation/restart | Existing real installed-MCP scenarios | Obsolete completion controls and extended crash cases |
| Scale/headroom/retention | Reusable scenarios and existing process sampling utilities | Backlog, RSS/disk attribution, finite retention and recovery measurements |

Use fixture repetition/generated modules at increasing sizes while preserving known call/import structure; record file/owner counts, source bytes, machine, revision, settings and samples. Do not claim 100 ms/500 ms thresholds or missing phase metrics. Add runtime instrumentation and scenario assertions as those production behaviors land.

Generate deterministic scale sources with `uv run --frozen --project codefabric-cpg-mcp python -m tooling.product.workload --output target/product/workspace-100 --modules 100`. The destination must be new. Generated module counts, source bytes and known call relationships are recorded beside the source; these are workload inputs, not measured runtime results. Register that workspace through the existing fixture/supervisor when the corresponding production adapter is wired.

`just golden --case python-live` exercises one running daemon through Python source changes.
`just golden --case mixed-clean-live` compares four implemented public query forms against independent
clean Python/Rust builds after call-target edits, compiler failure and repair. Separate state and
provider caches preserve the same authorized source identity; canonical IDs and semantic facts remain
in the comparison. Operational generations/provider runs and the snapshot-bound source-context handle
are excluded. This measured case has a 600 s default deadline; `--timeout` overrides it explicitly.
Contained providers require delegated cgroups and the configured provider binaries, as recorded in
[STATUS.md](../../STATUS.md). Broader edit/context and semantic coverage remain open.

`just golden --case staged-live` queries the first durable source/syntax epoch before semantic
publication, including named pending Rust targets. It holds a real completed semantic candidate,
queries current source and pending checker/compiler scope, checks a semantic deadline, replaces the held
candidate with a newer edit, and restarts from a durable pending source epoch. Its private debug-only
pause has cancellation and a deadline; release builds reject the fault configuration. This case also
has a 600 s harness bound for repeated contained builds.

`just golden --case processing-pages` queries 130 unfinished Python source partitions, consumes
the fact/manifest resources, restarts the daemon, repairs the workspace and reads the remaining
64-row and 2-row processing pages from the original exact snapshot. The paging tool resumes the
accepted daemon query ID and releases its separate resource after the final page. The default
case deadline is 600 seconds; `--timeout` overrides it.

`just golden --case python-context-live` changes the captured Python version/platform, adds and
removes a previously missing import, and selects an unsupported checker setting. Public declarations,
call occurrences, canonical targets/unknowns and processing scope are checked against independent
expectations and separate clean daemons. The initial inputs are restored to check identity recovery.
The case has the same 600-second default harness deadline and explicit timeout override.

`just golden --case python-stubs-live` retains same-name `.py` and `.pyi` declarations in a
namespace package, removes/restores the stub and verifies exact public call targets against their
source-file identities and separate clean builds. It has a 600-second default harness deadline.

`just golden --case python-roots-live` reverses/restores ordered import roots while retaining
all captured Python files. It checks that root-level scripts remain queryable and a root-level
module cannot shadow the configured roots, with independent expected targets and clean comparisons.
It has a 600-second default harness deadline.

`just golden --case python-paths-live` preserves raw and Unicode paths that collide under lossy
display conversion, filenames with URI metacharacters and a root initializer. It checks exact
file/entity identities, local calls and source spans through deletion/recreation and independent
clean builds. It has a 600-second default harness deadline.

`just golden --case decoded-source-live` checks original source bytes and checker-selected call
anchors for Latin-1 and UTF-8/BOM Python plus BOM/CRLF Rust. Encoding changes and restoration are
compared against independent clean daemons. The installed case passes (203.40 seconds on 2026-09-09)
and has a 600-second default harness deadline. Explicit zero-based UTF-8 and UTF-16 column
expectations, including an astral character, pass in 202.48 seconds; byte coordinates remain the
original captured positions. Text columns are null for an endpoint inside a character.

`just golden --case function-source-live` compares Python/Rust function definitions and bodies
against independently written source expectations and separate clean builds after edits/restoration.
It includes nested Python functions, CRLF/Unicode, a byte limit splitting a character and Rust braces
inside strings/comments. Its default harness deadline is 600 seconds.

`just golden --case source-lines-live` exercises explicit surrounding-line windows, CRLF,
Unicode and a missing final newline through installed clients and exact reopen. It distinguishes
an explicit byte prefix from non-retryable service hard-limit failure (33.70 seconds on 2026-09-09).
Its default harness deadline is 600 seconds.

`just golden --case rust-paths-live` exercises a Unicode Rust crate-root path alongside
non-UTF-8 and display-colliding Python paths. Declarations, calls and exact source pass through
the installed client after an edit, against an independent clean build and after exact reopen.
Its default harness deadline is 600 seconds.

`just golden --case cargo-build-live` runs a custom Cargo build script inside the selected
containment, then changes its cfg output. Installed declaration, call and source queries must
reflect the changed context and match an independent clean daemon. Build-script declarations
remain observable. Outgoing direct and known-empty callers use their admitted MIR owner scope;
incomplete build-script explanations carry the caller ID. Incoming queries retain broader scope,
and exact reopen preserves the selected results. Its default harness deadline is 600 seconds.

`just golden --case cargo-platforms-live` exercises captured Cargo platform selections, host
alias deduplication, missing-target scope, selected rustflags and extensionless configuration
precedence through installed queries, independent clean comparison and exact reopen. Its default
harness deadline is 600 seconds.

`just golden --case cargo-linkage-live` exercises `cdylib`, `dylib` and combined
`rlib`/`cdylib`/`staticlib` Cargo library targets through installed declaration, call and source
queries, live manifest changes, independent clean comparison and exact reopen. Its default
harness deadline is 600 seconds.

`just golden --case cargo-selections-live` exercises captured package/workspace feature and
build-profile selections, duplicate coalescing, independent valid and unavailable contexts, typed
selection remainders, live override, clean reconstruction and exact reopen through installed clients.
Its default harness deadline is 600 seconds.
