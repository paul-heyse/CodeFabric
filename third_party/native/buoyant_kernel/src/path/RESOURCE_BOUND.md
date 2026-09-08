# Native listing and retained path boundary (staged amendment)

This is an allocation/ownership seam in exact kernel 0.25.1, engine 0.25.0,
object_store 0.13.2, and url 2.5.8. It preserves the native filesystem listing,
filename parser, checkpoint grouping/selection, sorting, and transaction authority.
It is not proof of complete native resource closure.

## Original owned values

* `OwnedFileMeta` is move-only. Its private original `FileMeta` precedes its scope
  field. `try_from_object_path_admitted` reserves before cloning the base URL or
  formatting/applying the original object-store path. `into_parsed` moves that
  exact URL into the parsed owner; it does not replay or serialize a substitute.
* `ParsedLogPath` is an immutable Arc handle. Its original location, filename and
  extension are borrowed through `Deref`; callers cannot move an individual raw
  String or URL out of that shared owner. Clone shares the original Arc and needs
  no deep-copy admission. Compatibility parsing of a bare FileMeta/Url is rejected
  before copying under a required scope; the owned native ingress is explicit.
* `OwnedLogPaths` owns the actual original Vec allocation. Shared clone and its
  owned iterator preserve the Vec backing; there is no raw `into_vec` escape.
  Mutation on a shared collection, or under a new operation scope, first admits
  a fresh Vec and copies only immutable path handles. Unique growth reserves the
  full replacement Vec before `reserve_exact`. The original charge remains until
  its scope's last owner drops, including an exhausted iterator kept alive.
* `OwnedLogUrl` holds the native retained root URL and its scope together. Root
  and LogSegment clones share it. Explicit native raw URL-copy call sites reserve
  before native Url::clone; their enclosing synchronous operation owns their
  temporary lifetime. Those raw compatibility return APIs are separately noted
  below, not claimed as standalone owned output interfaces.
* `LogSegment` shares `Arc<LastCheckpointHint>`. The hint's original decode,
  embedded action/schema ownership and Arc-header admission belong to the
  separately implemented hint seam.

## Allocation geometry and ordering

All arithmetic uses checked layout helpers and rejects values above `isize::MAX`
before native allocation. Full new layouts are admitted; no in-place realloc
assumption or old/new net delta is used.

| Phase | Native source and request |
| --- | --- |
| Listing URL | url `lib.rs::set_path` clones serialization at exact current length, copies the query/fragment suffix, then mutates native parser serialization. `parser.rs::parse_path` percent-encodes each UTF-8 input byte into at most three bytes. Native String growth uses max(double old capacity, required, eight). The request includes original/replacement serialization coexistence, saved suffix, slash-prefixed formatting String, and the owner descriptor. This bound comes from uncompressed original path bytes, not physical file size. |
| Parsed path | Original URL path length bounds filename/extension String copies, staged UUID String parse, and split Vec. The Vec growth term includes actual `size_of::<&str>()` and minimum growth slots. Arc payload/header geometry is added before any original filename copying or collection. |
| Path collection | Checked full new capacity times `size_of::<ParsedLogPath>()`, plus actual Arc-inner/header geometry for new containers. Paths clone by shared immutable Arc, so descriptor admission does not falsely claim new String backing. |
| Native unordered listing | Every full new `Vec<OwnedFileMeta>` layout is reserved before growth. Original native `sort_unstable` is retained. The resulting iterator captures the scope through final iterator Drop; yielded items separately retain their original URL owner. |
| Checkpoint groups | Before native HashMap construction, reserve the map layout at the actual input count using the source-derived native hashbrown bucket/control layout helper. Group Vecs use OwnedLogPaths admission. Empty groups allocate nothing. Native grouping and equivalent-checkpoint choice are unchanged. |
| Backward scan | Each window uses an owned path collection; the outer Vec reserves each full replacement layout before growth. It retains windows until their owned iterators are consumed. |
| Generated relative joins | Native generated ASCII log names exclude absolute-URL, IDNA, query and fragment parsing. Base serialization plus relative length and native String minimum/geometric growth bound native relative join allocation. Constant/decimal filename formatting is separately admitted before format construction. |

The native receipt bank conservatively retains old layouts and transient charges
until the last scope owner. This is finite and observable, but is not an eviction
or scratch-reuse policy. Many-file histories need a suitably finite receipt-slot
policy; zero-byte empty checkpoint groups do not consume slots.

## Selected call sites and compatibility boundary

DefaultEngine filesystem listing captures the explicit/current scope before
crossing its TaskExecutor boundary. URL transformation and unordered Vec growth
use that captured owner even on an otherwise unconfigured background runtime.
MeteredStorageHandler forwards owned results. Normal forward, backward, hinted,
and timestamp-conversion listing feed OwnedFileMeta -> ParsedLogPath ->
OwnedLogPaths. The actual native listing remains the discovery source.

The native FileSystemCommitter returns OwnedFileMeta and `into_committed` transfers
it into the original parsed commit. Version-zero native transaction construction
continues through the admitted path. CRC/normal Protocol/Metadata replay consumes
these owned histories. Incremental pruning and native scan construction use
fallible owned collection copies/growth.

Legacy SyncStorageHandler and PlanBasedStorageHandler listing do not have the
selected preallocation path. They reject required scoped use before performing
legacy listing work, and preserve their ungoverned compatibility behavior. Bare
FileMeta/Url parsing and Vec ingress similarly carry no admission claim.

## Drop order and evidence

Rust drops struct fields in declaration order. Payload precedes owner in
OwnedFileMeta, ParsedLogPathFields, LogPathsInner and LogUrlInner. The same source
audit verifies Crc, CrcDelta, Metadata and Protocol owner-last fields;
OwnedCrcBytes is Vec then scope; ScopedCrcError/ActionError drop errors before
scope/diagnostic receipts. A deallocation observer in the external native probe
checks the actual original URL, filename and Vec allocations are freed before
receipt release. Clone/view/iterator tests use original pointer identity and
last-owner release; no copied byte or serialized projection is used as evidence.

External exact-graph probes: `probe/tests/listing_resource.rs` and
`probe/tests/crc_resource.rs`. These do not compile the native package's own
`cfg(test)` unit target. That target requires a separate dev-dependency harness;
Cargo rejects testing a dependency-only package from the shared probe.

## Explicit remaining obligations

This seam does not prove all native allocations. Open or separately owned paths
include object-store offset/prefix parsing and iterator/task boxing, generic
ObjectStore diagnostics, native FileMeta/URL copies in JSON/Parquet read setup,
sidecar discovery/manifest descriptors, the alternate sync/plan engines, generic
history/error output containers, and root URL ingress before native constructors.
Native raw-Url return compatibility APIs (commit path, checkpoint path, compaction
writer) are admitted temporary copies within a scoped operation; independently
retaining those raw values needs an owned output boundary.

The first-party governed constructor/caller inventory must remain explicit; a
legacy constructor's existence, a numeric structural limit, or an owner added
after native allocation does not prove that route is admitted. Arrow/Parquet
backing, generic expression/evaluation allocations, physical object-store disk
and memory, and joined native task ownership are separate amendments. No global
side map supplies table semantics or allocation lifetime authority.

## Read/sidecar continuation checkpoint

`OwnedFileMetas` now retains the actual read-selection Vec and each original
copied URL. Its consuming iterator yields move-only `OwnedFileMeta` and keeps the
Vec allocation until iterator drop. Native commit cover, checkpoint selections,
sidecar selections, and sequential-to-parallel transfer use this owner. The
selected Default JSON and Parquet readers preadmit their caller-slice copy;
per-file streams borrow the retained URL string instead of allocating a detached
file-location String. Presigned FileStream stores the owned iterator directly.
The generic presigned HTTP client and response allocations remain separately
uncovered deployment paths, not covered by the local filesystem proof.

`SidecarVisitor` borrows original path/tags for admission before native getters
copy them, reserves each full replacement descriptor Vec, and attaches an
owner-last field to each original Sidecar. Legacy derived Clone clears the owner;
`try_clone_admitted` reserves a full new map/string copy. As with existing public
native DTO compatibility fields, moving a Sidecar public field independently
is not an owned escape API; the selected replay keeps the original action alive
until an independently admitted original URL is moved into OwnedFileMetas.

`path::url_resource` supplies an explicit deployment callback, installed per
native worker with `NativeUrlThreadPolicy`. `LocalUrlJoinAdmission` is configured
from authorized local directory URLs and a finite uncompressed reference-byte
limit. Before native Url::join it scans borrowed Unicode input without allocation:
url2.5.8 Input trims edge C0/space and ignores tab/CR/LF everywhere; scheme parsing
is ASCII case-insensitive; both slash and backslash start file authority. Only
empty and ASCII localhost (including percent-encoded ASCII bytes) authorities
can reach native file-host parsing. Other authorities/schemes are rejected before
unbounded Unicode host/IDNA work. Legal Unicode path bytes remain native input.

The selected file parser's maximum serialization is bounded by original base
bytes plus three times original reference UTF-8 bytes plus fixed separators.
Its scheme scan may fill then clear serialization; file/relative parsing copies
the base prefix, percent-encodes paths/queries/fragments, and native file path
normalization performs one `split_off` path copy. Cumulative complete geometric
String layouts, that simultaneous split copy, ignored-host-character collection,
and the nine-byte localhost case-fold output are admitted before join. IDNA1.1
handles the one ASCII9 localhost label inside its inline `[char;253]` and
`[AlreadyAsciiLabel;8]` buffers; there is no arbitrary Unicode hostname path.
After native normalization the result must still be a hostless/queryless/
fragmentless file URL under a configured URL-root boundary. Decoded physical
paths, symlink/device checks, and descriptor identity remain independently
validated by the workspace OwnedLocalStore; URI checks do not replace those.

The URL callback remains extensible for other deployments. Missing required
callback is a typed failure before join. Policy installation succeeds on an
already-failed scope, so late blocking workers still install every guard; actual
work checks the sticky failure before parsing. A foreign nested scope is denied.

Engine `owned_box_stream` admits the concrete `size_of_val` layout before Box
allocation and retains its scope in a futures Map closure (native Map declares
stream before closure). The outer BlockingStreamIterator similarly reserves its
concrete layout and declares its owner last. Object-store offset/prefix setup
now prepays percent-decoded/path Strings, diagnostics, and PathPart Vec growth
using original encoded URL length (encoded `%2f` can add separators).

Engine `resource::owned_buffered` is grounded in exact futures-util0.3.34 with
standard Arc/atomics (resolved graph omits portable-atomic-alloc). Buffered's
maximum counts both live futures and completed queued outputs, while try_flatten
retains one extra current file stream. Admission prepays the *total source file
count* Task<OrderWrapper<concrete future>> nodes, plus the same-typed stub Task,
ReadyToRunQueue Arc, and cumulative full BinaryHeap output layouts. Field-wise
maximum-alignment padding bounds private repr(Rust) ordering; no raw native
layout is exposed or mutated. The source total is the exact owning input Vec
length, never an estimate from active I/O. Typed denial precedes Buffered::new.
External cloned Wakers may retain completed nodes after logical completion;
the whole operation scope must therefore remain alive until the native lane's
runtime and physical workers join. This test checkpoint proves preadmission and
input/stream ownership, not that independent join barrier.

Still open in this continuation: exact TaskExecutor per-call native channel/
Tokio task admission, generic ObjectStore and HTTP diagnostic/payload paths,
raw FileSlice/URL output compatibility, actual root URL creation before native
ingress, arbitrary Delta history/operation output vectors, Delta writer rolling
JoinSet total-file admission, and independent retained external-Waker audit.
