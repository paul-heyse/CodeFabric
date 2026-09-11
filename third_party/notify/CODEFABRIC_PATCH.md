# CodeFabric notify 8.2.0 ownership patch

Source: crates.io `notify` 8.2.0, checksum
`4d3d07927151ff8575b7087f245456e549fea62edf0ec4e565a5ee50c8402bc3`.
The upstream VCS selection is retained in `.cargo_vcs_info.json`; the original manifest and CC0
license are preserved. The root Cargo patch selects this source without upgrading its version.

Upstream polling and Linux inotify spawn their workers and discard the `JoinHandle`. A debouncer
join therefore cannot join those workers. This patch retains both handles and propagates spawn
failure. Construction-only `Config::with_join_on_drop(true)` opts an ordinary owner into backend
joining; the upstream default remains false. Polling stop wakes timed and manual waits. Linux
shutdown tolerates an already closed event loop, then joins, rather than panicking before cleanup.
A worker panic is logged after the actual join. Event callbacks must not drop their own joined
watcher. A blocking scan/callback must finish before joined cleanup can return.

CodeFabric sets this option on its existing owned blocking watch lifecycle and continues to call
`Debouncer::stop()`. There is no new application watcher implementation, service or executor.
The default API and other platform backends retain upstream behavior. The public upstream
`stop_nonblocking()` documentation applies with upstream/default backend configuration; CodeFabric
uses explicit joined construction and does not call that method.

The root consumer tests cover manual/timed polling, in-flight callbacks, Linux inotify, topology
repair and cancellation. This is a scoped Linux/polling ownership extension; it does not claim
joined Windows, kqueue or FSEvents backends or filesystem event completeness.
