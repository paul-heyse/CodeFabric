use std::{alloc::{GlobalAlloc,Layout,System},cell::Cell,future::Future,pin::Pin,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}},task::{Context,Poll}};
use tokio::runtime::resource::*;
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
 unsafe fn alloc(&self,layout:Layout)->*mut u8{if TRACK.try_with(|v|v.get()).unwrap_or(false){let _=ALLOCATIONS.try_with(|v|v.set(v.get()+1));}unsafe{System.alloc(layout)}}
 unsafe fn dealloc(&self,p:*mut u8,layout:Layout){unsafe{System.dealloc(p,layout)}}
 unsafe fn realloc(&self,p:*mut u8,layout:Layout,n:usize)->*mut u8{if TRACK.try_with(|v|v.get()).unwrap_or(false){let _=ALLOCATIONS.try_with(|v|v.set(v.get()+1));}unsafe{System.realloc(p,layout,n)}}
}
#[global_allocator]static ALLOCATOR:Allocator=Allocator;
#[derive(Debug)]struct Receipt(usize);
impl RuntimeAllocationReceipt for Receipt{fn bytes(&self)->usize{self.0}}
#[derive(Debug)]struct Admission{deny:AtomicBool,failures:AtomicUsize,requests:Mutex<Vec<RuntimeAllocationRequest>>}
impl Admission{fn new()->Self{Self{deny:AtomicBool::new(false),failures:AtomicUsize::new(0),requests:Mutex::new(Vec::with_capacity(256))}}}
impl RuntimeAllocationAdmission for Admission{
 fn try_reserve(&self,request:RuntimeAllocationRequest)->Result<Arc<dyn RuntimeAllocationReceipt>,ResourceLayoutError>{self.requests.lock().unwrap().push(request);if self.deny.load(Ordering::SeqCst){Err(ResourceLayoutError{kind:"native_test_deny",requested:request.bytes,limit:0})}else{Ok(Arc::new(Receipt(request.bytes)))}}
 fn record_failure(&self,_:ResourceLayoutError){self.failures.fetch_add(1,Ordering::SeqCst);}
}
fn profile()->LocalRuntimeProfile{LocalRuntimeProfile{worker_threads:1,blocking_threads:2,blocking_queue:8,thread_stack_bytes:2<<20,async_tasks:128,blocking_tasks:128}}
struct LargeFuture{_bytes:[u8;32768],polled:Arc<AtomicUsize>}
impl Future for LargeFuture{type Output=();fn poll(self:Pin<&mut Self>,_:&mut Context<'_>)->Poll<()>{self.polled.fetch_add(1,Ordering::SeqCst);Poll::Ready(())}}
fn observed<T>(f:impl FnOnce()->T)->(T,usize){ALLOCATIONS.with(|v|v.set(0));TRACK.with(|v|v.set(true));let value=f();TRACK.with(|v|v.set(false));(value,ALLOCATIONS.with(|v|v.get()))}
#[test]fn large_async_denial_has_no_future_box_task_or_error_allocation(){
 let admission=Arc::new(Admission::new());let runtime=profile().build(admission.clone(),||{},||{}).unwrap();let _entered=runtime.enter();let polled=Arc::new(AtomicUsize::new(0));let future=LargeFuture{_bytes:[0;32768],polled:polled.clone()};admission.deny.store(true,Ordering::SeqCst);
 let ((task,abort),allocations)=observed(||{let task=tokio::spawn(future);let abort=task.abort_handle();(task,abort.clone())});assert_eq!(allocations,0);assert!(task.is_finished());assert_eq!(abort.id(),task.id());assert_eq!(polled.load(Ordering::SeqCst),0);let error=runtime.block_on(task).unwrap_err();assert!(error.resource_error().is_some());assert_eq!(admission.failures.load(Ordering::SeqCst),1);
}
#[test]fn large_blocking_denial_precedes_closure_box_and_native_thread_creation(){
 let admission=Arc::new(Admission::new());let runtime=profile().build(admission.clone(),||{},||{}).unwrap();let bytes=[4u8;32768];let polled=Arc::new(AtomicUsize::new(0));let marker=polled.clone();let closure=move||{std::hint::black_box(bytes);marker.fetch_add(1,Ordering::SeqCst);};admission.deny.store(true,Ordering::SeqCst);
 let (task,allocations)=observed(||runtime.spawn_blocking(closure));assert_eq!(allocations,0);assert!(task.is_finished());assert_eq!(polled.load(Ordering::SeqCst),0);assert!(runtime.block_on(task).unwrap_err().resource_error().is_some());
}
#[test]fn fallible_block_on_denies_before_original_large_future_box_and_poll(){
 let admission=Arc::new(Admission::new());let runtime=profile().build(admission.clone(),||{},||{}).unwrap();let polled=Arc::new(AtomicUsize::new(0));let future=LargeFuture{_bytes:[0;32768],polled:polled.clone()};admission.deny.store(true,Ordering::SeqCst);
 let (result,allocations)=observed(||runtime.try_block_on(future));assert_eq!(allocations,0);assert!(result.is_err());assert_eq!(polled.load(Ordering::SeqCst),0);
}
struct CountWake(AtomicUsize);impl std::task::Wake for CountWake{fn wake(self:Arc<Self>){self.0.fetch_add(1,Ordering::SeqCst);}fn wake_by_ref(self:&Arc<Self>){self.0.fetch_add(1,Ordering::SeqCst);}}
#[test]fn distinct_deferred_wakers_over_inline_capacity_allocate_nothing(){
 let admission=Arc::new(Admission::new());let runtime=profile().build(admission,||{},||{}).unwrap();let counters:Vec<_>=(0..96).map(|_|Arc::new(CountWake(AtomicUsize::new(0)))).collect();let wakers:Vec<_>=counters.iter().cloned().map(std::task::Waker::from).collect();
 let (immediate,allocated)=runtime.block_on(runtime.spawn(async move{
     observed(||{for waker in &wakers {let mut cx=Context::from_waker(waker);let mut future=std::pin::pin!(tokio::task::yield_now());assert!(future.as_mut().poll(&mut cx).is_pending());} counters.iter().map(|c|c.0.load(Ordering::SeqCst)).sum::<usize>()})
 })).unwrap();assert_eq!(allocated,0);assert!(immediate>=64);
}
#[test]fn joined_handle_cannot_allocate_or_poll_new_native_task(){
 let admission=Arc::new(Admission::new());let runtime=profile().build(admission.clone(),||{},||{}).unwrap();let handle=runtime.handle().clone();drop(runtime);
 let polled=Arc::new(AtomicUsize::new(0));let future=LargeFuture{_bytes:[0;32768],polled:polled.clone()};
 let (mut task,allocations)=observed(||handle.spawn(future));assert_eq!(allocations,0);assert!(task.is_finished());
 let waker=std::task::Waker::noop();let mut cx=Context::from_waker(waker);let error=match Pin::new(&mut task).poll(&mut cx){Poll::Ready(Err(error))=>error,_=>panic!("expected inline refusal")};
 assert_eq!(error.resource_error().unwrap().kind,"tokio_runtime_shutdown");assert_eq!(polled.load(Ordering::SeqCst),0);
}
