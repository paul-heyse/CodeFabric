//! Native futures-util 0.3.34 descriptor admission. These are source-layout
//! upper bounds, not payload bounds or a limit on the amount of decoded data.
use std::mem::{align_of, size_of};
use std::sync::Arc;
use delta_kernel::resource::{AllocationRequest, NativeResourceScope, ResourceExhausted};
use delta_kernel::DeltaResult;
use futures::{Future, Stream, StreamExt};

fn overflow() -> delta_kernel::Error {
    ResourceExhausted { kind: "native_future_descriptors", requested: usize::MAX, limit: isize::MAX as usize }.into()
}
fn add(a: usize, b: usize) -> DeltaResult<usize> { a.checked_add(b).filter(|v| *v <= isize::MAX as usize).ok_or_else(overflow) }
fn mul(a: usize, b: usize) -> DeltaResult<usize> { a.checked_mul(b).filter(|v| *v <= isize::MAX as usize).ok_or_else(overflow) }
fn padded(value: usize, align: usize) -> DeltaResult<usize> { Ok(add(value, align - 1)? & !(align - 1)) }
// Padding each field to the maximum field alignment bounds any native Rust
// field order; it does not assume the private upstream repr(Rust) order.
fn fields(fields: &[(usize, usize)]) -> DeltaResult<(usize, usize)> {
    let align = fields.iter().map(|(_, align)| *align).max().unwrap_or(1);
    let mut bytes = 0;
    for (size, _) in fields { bytes = add(bytes, padded(*size, align)?)?; }
    Ok((bytes, align))
}
fn arc(payload: (usize, usize)) -> DeltaResult<usize> {
    fields(&[(2 * size_of::<usize>(), align_of::<usize>()), payload]).map(|(bytes, _)| bytes)
}
fn order<T>() -> DeltaResult<(usize, usize)> {
    fields(&[(size_of::<T>(), align_of::<T>()), (size_of::<i64>(), align_of::<i64>())])
}
fn task<F: Future>() -> DeltaResult<usize> {
    let (ordered, align) = order::<F>()?;
    // Task.future is Option<OrderWrapper<F>>. Even without niche optimization,
    // one full aligned tag slot bounds the two-variant discriminant layout.
    let option = add(ordered, align)?;
    let pointer = (size_of::<usize>(), align_of::<usize>());
    arc(fields(&[
        (option, align), pointer, pointer, pointer, pointer,
        (size_of::<std::sync::Weak<()>>(), align_of::<std::sync::Weak<()>>()),
        (size_of::<std::sync::atomic::AtomicBool>(), align_of::<std::sync::atomic::AtomicBool>()),
        (size_of::<std::sync::atomic::AtomicBool>(), align_of::<std::sync::atomic::AtomicBool>()),
    ])?)
}

/// Prepay every task node this finite source can create, including completed
/// futures retained by cloned Wakers. Never infer this total from concurrency.
///
/// Native Buffered enforces logical in-flight+queued-output count <= maximum.
/// Its BinaryHeap retains capacity and may geometrically grow; four times the
/// larger of maximum and native minimum capacity includes the cumulative full
/// new layouts. Payload allocations remain separately owned/admitted.
pub(crate) fn owned_buffered<S>(
    stream: S,
    maximum: usize,
    total: usize,
    scope: Option<&Arc<NativeResourceScope>>,
) -> DeltaResult<futures::stream::Buffered<S>>
where S: Stream, S::Item: Future,
{
    if let Some(scope) = scope {
        if maximum == 0 {
            return Err(ResourceExhausted { kind: "native_read_ahead", requested: 1, limit: 0 }.into());
        }
        let task = task::<S::Item>()?;
        let ready_queue = arc(fields(&[
            (size_of::<futures::task::AtomicWaker>(), align_of::<futures::task::AtomicWaker>()),
            (size_of::<usize>(), align_of::<usize>()),
            (size_of::<usize>(), align_of::<usize>()),
            (size_of::<Arc<()>>(), align_of::<Arc<()>>()),
        ])?)?;
        // FuturesUnordered::new allocates one same-typed stub Task as well.
        let nodes = mul(add(total, 1)?, task)?;
        let heap_slots = if total == 0 { 0 } else { mul(maximum.min(total).max(4), 4)? };
        let heap = mul(heap_slots, order::<<S::Item as Future>::Output>()?.0)?;
        scope.reserve(AllocationRequest { kind: "native_buffered_total_nodes", bytes: add(add(nodes, ready_queue)?, heap)? })?;
    }
    Ok(stream.buffered(maximum))
}

/// object_store 0.13.2 Path::from_url_path percent-decodes at most input.len
/// bytes then copies the stripped path. BadSegment may own both the complete
/// path and a segment; UTF-8/illegal-character diagnostics fit the same source
/// bound. Each full String replacement layout is included before parsing.
pub(crate) fn admit_object_path(
    path: &str,
    scope: Option<&Arc<NativeResourceScope>>,
) -> DeltaResult<()> {
    if let Some(scope) = scope {
        scope.reserve(AllocationRequest {
            kind: "native_object_path_parse",
            bytes: add(mul(add(path.len(), 8)?, 4)?, size_of::<delta_kernel::object_store::path::Path>())?,
        })?;
    }
    Ok(())
}

pub(crate) fn admit_listing_prefix(
    url: &url::Url,
    scope: Option<&Arc<NativeResourceScope>>,
) -> DeltaResult<()> {
    admit_object_path(url.path(), scope)?;
    if let Some(scope) = scope {
        // Percent decoding may turn %2f into '/', so original byte length+1
        // bounds the number of borrowed PathPart descriptors before decoding.
        // Vec grows geometrically from a zero lower size hint; sum of full
        // new layouts is below four times max(required, minimum4).
        let parts = mul(add(url.path().len(), 1)?.max(4), 4)?;
        let descriptors = mul(parts, size_of::<delta_kernel::object_store::path::PathPart<'_>>())?;
        let strings = mul(add(url.as_str().len(), 64)?, 4)?;
        scope.reserve(AllocationRequest { kind: "native_listing_prefix", bytes: add(descriptors, strings)? })?;
    }
    Ok(())
}

/// Full std1.98 one-message mpsc channel layouts before channel construction.
/// The two CachePadded Positions use at most256-byte alignment on supported
/// native targets. The separate worker Thread backing belongs to runtime policy.
pub(crate) fn admit_executor_channel<T>(scope: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if let Some(scope) = scope {
        let word = (size_of::<usize>(), align_of::<usize>());
        let position = (padded(2 * size_of::<usize>(), 256)?, 256);
        let waker = (size_of::<std::sync::Mutex<[Vec<[usize; 3]>; 2]>>(), align_of::<std::sync::Mutex<[Vec<[usize; 3]>; 2]>>());
        let channel = fields(&[position, position, waker, (1, 1)])?;
        let counter = fields(&[word, word, (1, 1), channel])?.0;
        let slot = fields(&[(size_of::<T>(), align_of::<T>()), word])?;
        let block = fields(&[word, (mul(slot.0, 31)?, slot.1)])?.0;
        // There is one receiver. Both native Waker vectors are conservatively
        // given their minimum4 slots, though this path uses only selectors.
        let registrations = 8 * size_of::<[usize; 3]>();
        let context = arc(fields(&[word, word, (size_of::<std::thread::Thread>(), align_of::<std::thread::Thread>()), word])?)?;
        scope.reserve(AllocationRequest { kind: "native_executor_channel", bytes: add(add(add(counter, block)?, add(registrations, context)?)?, "native Tokio executor terminated before returning its admitted result".len())? })?;
    }
    Ok(())
}

pub(crate) fn admit_executor_relay<F: Future<Output = ()>>(
    future: &F,
    scope: Option<&Arc<NativeResourceScope>>,
) -> DeltaResult<()> {
    if let Some(scope) = scope {
        native_profile()?;
        // This Box is created before handing the future to Tokio. The native
        // installed hook separately pays the actual enclosing task Cell.
        scope.reserve(AllocationRequest { kind: "native_executor_relay", bytes: std::mem::size_of_val(future) })?;
    }
    Ok(())
}

pub(crate) fn admit_executor_send<T>(scope: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if scope.is_some() {native_profile()?;}
    Ok(())
}

/// Full original Arc allocation including its strong/weak counters.
pub(crate) fn admit_arc<T>(kind: &'static str, scope: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if let Some(scope) = scope {
        scope.reserve(AllocationRequest { kind, bytes: arc((size_of::<T>(), align_of::<T>()))? })?;
    }
    Ok(())
}


fn validate_worker(scope: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if let Some(scope) = scope {
        let current = delta_kernel::resource::current_resource_scope();
        if current.as_ref().is_none_or(|current| !Arc::ptr_eq(current, scope)) {
            let error = ResourceExhausted { kind: "native_task_worker_scope", requested: 1, limit: 0 };
            scope.record_failure(error);
            return Err(error.into());
        }
        scope.check_available()?;
    }
    Ok(())
}

fn admit_task<F: Future>(_future: &F, scope: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if scope.is_some() {native_profile()?;}
    Ok(())
}

/// Pre-admit each actual native task, including tasks already completed but
/// retained by JoinHandles or external Wakers. The enclosing runtime owner
/// must keep the operation scope until its shutdown/join barrier completes.
pub fn try_spawn<F, T, E>(future: F) -> DeltaResult<tokio::task::JoinHandle<Result<T, E>>>
where F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static, E: From<delta_kernel::Error> + Send + 'static,
{
    let scope = delta_kernel::resource::current_resource_scope();
    validate_worker(scope.as_ref())?;
    let task_scope = scope.clone();
    let guarded = async move {
        validate_worker(task_scope.as_ref()).map_err(E::from)?;
        future.await
    };
    admit_task(&guarded, scope.as_ref())?;
    Ok(tokio::spawn(guarded))
}

/// Native JoinSet with descriptor ownership and fallible admission before every
/// spawn. A clone of an old scope never pays for a new task or list entry.
#[derive(Debug)]
pub struct OwnedJoinSet<T> {
    inner: tokio::task::JoinSet<T>,
    scope: Option<Arc<NativeResourceScope>>,
}
impl<T: 'static> OwnedJoinSet<T> {
    pub fn try_new() -> DeltaResult<Self> {
        let scope = delta_kernel::resource::current_resource_scope();
        if let Some(scope) = &scope {
            native_profile()?;
            scope.reserve(AllocationRequest { kind: "native_join_set_lists", bytes: tokio::runtime::resource::join_set_new_bytes::<T>().map_err(native_error)? })?;
        }
        Ok(Self { inner: tokio::task::JoinSet::new(), scope })
    }
    pub async fn join_next(&mut self) -> Option<Result<T, tokio::task::JoinError>> { self.inner.join_next().await }
    pub fn abort_all(&mut self) { self.inner.abort_all(); }
    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }
}
impl<T, E> OwnedJoinSet<Result<T, E>>
where T: Send + 'static, E: From<delta_kernel::Error> + Send + 'static,
{
    pub fn spawn<F>(&mut self, future: F) -> DeltaResult<tokio::task::AbortHandle>
    where F: Future<Output = Result<T, E>> + Send + 'static,
    {
        validate_worker(self.scope.as_ref())?;
        let task_scope = self.scope.clone();
        let guarded = async move {
            validate_worker(task_scope.as_ref()).map_err(E::from)?;
            future.await
        };
        // Native ListEntry owns two linked-list pointers, an Arc<Lists>,
        // JoinHandle<T>, a three-variant List tag, and ZST PhantomPinned.
        if let Some(scope) = &self.scope {
            // One exclusive abort traversal may allocate a pointer Vec up to
            // current len. Prepay one pointer per admitted entry; subsequent
            // abort calls reuse that envelope after the prior Vec was dropped.
            let entry = tokio::runtime::resource::join_set_entry_bytes::<Result<T,E>>().map_err(native_error)?;
            let abort = tokio::runtime::resource::join_set_abort_bytes::<Result<T,E>>(1).map_err(native_error)?;
            scope.reserve(AllocationRequest { kind: "native_join_set_entry", bytes: add(entry,abort)? })?;
        }
        admit_task(&guarded, self.scope.as_ref())?;
        self.inner.try_spawn(guarded).map_err(native_error)
    }
}

#[derive(Debug)]
struct ChannelOwner {
    attempted_sends: std::sync::atomic::AtomicUsize,
    scope: Option<Arc<NativeResourceScope>>,
}

/// Original Tokio sender plus admission owner. Clone shares both backing owners.
#[derive(Debug)]
pub struct OwnedSender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
    owner: Arc<ChannelOwner>,
}
impl<T> Clone for OwnedSender<T> {
    fn clone(&self) -> Self { Self { inner: self.inner.clone(), owner: self.owner.clone() } }
}
/// The receiver retains all native list-block and channel-header charges.
#[derive(Debug)]
pub struct OwnedReceiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
    owner: Arc<ChannelOwner>,
}
impl<T> OwnedReceiver<T> {
    pub async fn recv(&mut self) -> DeltaResult<Option<T>> {
        validate_worker(self.owner.scope.as_ref())?;
        let value = self.inner.recv().await;
        validate_worker(self.owner.scope.as_ref())?;
        Ok(value)
    }
    pub fn close(&mut self) { self.inner.close(); }
}
fn channel_block<T>() -> DeltaResult<usize> {
    // Tokio mpsc::block has four word header fields and BLOCK_CAP inline
    // MaybeUninit<T> slots:32 on64bit,16 on32bit.32 bounds both.
    fields(&[(4 * size_of::<usize>(), align_of::<usize>()),
        (mul(32, size_of::<T>())?, align_of::<T>())]).map(|(bytes, _)| bytes)
}

/// Construct the native bounded channel only after header, initial/close block,
/// and ownership-Arc admission. Capacity bounds buffered values; cumulative
/// send admission separately covers every possible allocation/reuse race.
pub fn try_channel<T>(capacity: usize) -> DeltaResult<(OwnedSender<T>, OwnedReceiver<T>)> {
    let scope = delta_kernel::resource::current_resource_scope();
    if capacity == 0 || capacity > tokio::sync::Semaphore::MAX_PERMITS {
        return Err(ResourceExhausted { kind: "native_channel_capacity", requested: capacity, limit: tokio::sync::Semaphore::MAX_PERMITS }.into());
    }
    if let Some(scope) = &scope {
        let word = (size_of::<usize>(), align_of::<usize>());
        let header = fields(&[
            (padded(2 * size_of::<usize>(), 256)?, 256),
            (padded(size_of::<futures::task::AtomicWaker>(), 256)?, 256),
            (size_of::<tokio::sync::Notify>(), align_of::<tokio::sync::Notify>()),
            (size_of::<tokio::sync::Semaphore>(), align_of::<tokio::sync::Semaphore>()),
            word, word, word, // semaphore bound + strong/weak sender counts
            (4 * size_of::<usize>(), align_of::<usize>()), // Rx list3word + bool
        ])?;
        let bytes = add(add(arc(header)?, mul(2, channel_block::<T>()?)?)?,
            arc((size_of::<ChannelOwner>(), align_of::<ChannelOwner>()))?)?;
        scope.reserve(AllocationRequest { kind: "native_channel_header", bytes })?;
    }
    let owner = Arc::new(ChannelOwner { attempted_sends: std::sync::atomic::AtomicUsize::new(0), scope });
    let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
    Ok((OwnedSender { inner: sender, owner: owner.clone() }, OwnedReceiver { inner: receiver, owner }))
}
impl<T> OwnedSender<T> {
    /// Charge before native permit wait / enqueue. The error message is static
    /// and its exact owned diagnostic layout is pre-admitted before the send.
    pub async fn send(&self, value: T, closed_message: &'static str) -> DeltaResult<()> {
        validate_worker(self.owner.scope.as_ref())?;
        let attempt = self.owner.attempted_sends.fetch_update(std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| overflow())?;
        if let Some(scope) = &self.owner.scope {
            // find_block can walk from any earlier block and contenders can
            // allocate losing candidates. Each attempt admits all blocks up to
            // its cumulative ordinal, plus a final close block. Reordering
            // permit acquisition only permutes ordinals; the operation retains
            // every earlier admission. This deliberately overpays recycled
            // blocks rather than assuming the bounded queue stops allocation.
            let blocks = add(attempt / 16, 2)?;
            let bytes = add(mul(blocks, channel_block::<T>()?)?, closed_message.len())?;
            scope.reserve(AllocationRequest { kind: "native_channel_send_blocks", bytes })?;
        }
        let permit = self.inner.reserve().await.map_err(|_| delta_kernel::Error::Generic(closed_message.to_owned()))?;
        validate_worker(self.owner.scope.as_ref())?;
        permit.send(value);
        Ok(())
    }
}

/// Source layout for a native external JoinSet's cumulative tasks. This is used
/// when the original library owns the JoinSet and its exact Future type is public.
/// It does not admit work; the caller must reserve the returned full new layout
/// before delegating any call that can create those native nodes.
pub fn external_join_set_bytes<F: Future>(tasks: usize, include_header: bool) -> DeltaResult<usize> {
    native_profile()?;
    let entry=tokio::runtime::resource::join_set_entry_bytes::<F::Output>().map_err(native_error)?;
    let abort=tokio::runtime::resource::join_set_abort_bytes::<F::Output>(tasks).map_err(native_error)?;
    let mut bytes=add(mul(tasks,entry)?,abort)?;
    if include_header {bytes=add(bytes,tokio::runtime::resource::join_set_new_bytes::<F::Output>().map_err(native_error)?)?;}
    // Every actual Cell and future box is reserved by the original installed
    // Tokio spawn boundary, before allocation; do not duplicate guessed Cells.
    Ok(bytes)
}

fn native_error(error:tokio::runtime::resource::ResourceLayoutError)->delta_kernel::Error{
    ResourceExhausted{kind:error.kind,requested:error.requested,limit:error.limit}.into()
}
fn native_profile()->DeltaResult<()>{
    tokio::runtime::resource::check_current_profile().map_err(native_error).map(|_|())
}
