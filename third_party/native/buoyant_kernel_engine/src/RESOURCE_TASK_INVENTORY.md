# Exact native task/queue inventory (staged, not closure certification)

Versions: kernel engine0.25.0; futures-util0.3.34; Tokio1.53.1; Rust1.98;
Delta43a0cf10. Standard Arc/atomic futures feature path is selected.

| Native path | Actual bound and allocation/lifetime issue |
| --- | --- |
| JSON/Parquet Buffered | Builder `with_buffer_size(NonZero)`; default1000. `FuturesOrdered::len` sums in-progress and completed BinaryHeap outputs; <=n logical nodes, plus try_flatten's current stream. The added preadmission charges total input files, not just n, plus native stub and ready queue; copied Wakers can retain old completed nodes. |
| Storage read_files | Independent `with_readahead(usize)` default10; legacy accepts0. Buffered native semantics as above, but this route still needs owned FileSlice ingress and helper migration. |
| Presigned FileStream | One active reader plus one `NextOpen` Pending/Ready; original descriptor Vec now remains owned. reqwest client/HTTP body allocations are outside local selected profile. |
| TokioMultiThreadExecutor block_on | Per call creates std::mpsc::channel<T::Output>, boxes relay future, spawns one async task, then one spawn_blocking send task. Exactly one channel message; the channel type is unbounded but actual send count is1. Every iterator.next call repeats these allocations; worker count does not bound queued calls. |
| TokioBackgroundExecutor | Constructor channel50 queues BoxFuture, but receiver drains them into independent Tokio tasks. Fifty is not a cap on total live/completed task nodes. Startup also std channel<Handle> and an owned runtime thread. |
| Delta ReceiverStreamBuilder | Fixed channel100 at scan_metadata/from/tombstones; one blocking producer each. Up to100 queued outputs plus blocked producer's value plus consumer-held output. JoinSet drop does not join running blocking work; owned lane shutdown barrier is required. |
| Delta unpartitioned write_streams | One worker per physical input stream plus one writer. Channel size reads DELTARS_WRITER_BATCH_CHANNEL_SIZE OnceLock first use, default10, arbitrary positiveusize accepted. Partitioned writer cap does not constrain this stream count. |
| Delta partitioned write | max_concurrent_writers first-use OnceLock, env DELTARS_MAX_CONCURRENT_WRITERS, defaultnumcpus, clamp1..128; cap applied in partitioned repartition helper. |
| Delta PartitionWriter rolls | Each rolled file spawns in_flight_writers task; JoinSet only drained at close. Completed upload metadata/tasks can accumulate to total rolled files. Requires finite total-roll preadmission or native bounded drain, independent of multipart request concurrency. |
| Delta multipart knobs | UPLOAD_PART_SIZE first-use OnceLock env default5MiB clamp5MiB..5GiB; MAX_CONCURRENCY_TASKS first-use OnceLock default10 accepts arbitraryusize including0, and config override. `DeltaWriter::close` buffers partition closes at num_cpus::get. |

TaskExecutor channel source: std1.98 sync/mpmc/list.rs `Counter<Channel<T>>`
allocation; head/tail CachePadded Position fields; receivers SyncWaker Mutex of
two Vec<Entry>; one Block with31 Slot<T> values for this one-message channel.
Blocked receiver registration allocates at most its selector Vec minimum4 Entry;
Entry holds Operation, packet pointer, Context Arc. Context has cached thread TLS
Arc<Inner{selected,packet,Thread,thread_id}>. That Context and Thread backing must
remain charged through worker exit, not just successful recv.

Tokio task source: runtime/task/core.rs Cell<T,S> holds Header, Core scheduler/id/
Stage future-or-output, Trailer owned-list links/Option<Waker>/termination hook.
Cell is cache aligned128 on selected x86_64 (256 on s390x); features/test-util/
tokio_unstable affect exact fields. BlockingTask contains Option<closure> and
BlockingSchedule retains hooks (plus Handle under test-util). Source-derived
admission must include exact concrete relay/closure future layouts and selected
feature variants before channel/box/spawn. No nominal worker envelope is claimed
here as proof of these per-call allocations.
