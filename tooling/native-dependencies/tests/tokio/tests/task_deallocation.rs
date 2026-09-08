use std::{alloc::{GlobalAlloc,Layout,System},sync::{Arc,atomic::{AtomicBool,AtomicPtr,AtomicUsize,Ordering}}};
use tokio::runtime::resource::*;
static EXPECTED_CELL_SIZE:AtomicUsize=AtomicUsize::new(0);
static CELL_POINTER:AtomicPtr<u8>=AtomicPtr::new(std::ptr::null_mut());
static RECEIPT_LIVE:AtomicBool=AtomicBool::new(false);
static LIVE_AT_DEALLOC:AtomicBool=AtomicBool::new(false);
static CELL_DEALLOCATED:AtomicBool=AtomicBool::new(false);
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
 unsafe fn alloc(&self,layout:Layout)->*mut u8{
  let p=unsafe{System.alloc(layout)};
  if layout.size()==EXPECTED_CELL_SIZE.load(Ordering::SeqCst) && layout.align()==128 {
   let _=CELL_POINTER.compare_exchange(std::ptr::null_mut(),p,Ordering::SeqCst,Ordering::SeqCst);
  }
  p
 }
 unsafe fn dealloc(&self,p:*mut u8,layout:Layout){
  if p==CELL_POINTER.load(Ordering::SeqCst){
   LIVE_AT_DEALLOC.store(RECEIPT_LIVE.load(Ordering::SeqCst),Ordering::SeqCst);
   CELL_DEALLOCATED.store(true,Ordering::SeqCst);
  }
  unsafe{System.dealloc(p,layout)}
 }
}
#[global_allocator]static ALLOCATOR:Allocator=Allocator;
#[derive(Debug)]struct Receipt{bytes:usize,task:bool}
impl RuntimeAllocationReceipt for Receipt{fn bytes(&self)->usize{self.bytes}}
impl Drop for Receipt{fn drop(&mut self){if self.task{RECEIPT_LIVE.store(false,Ordering::SeqCst);}}}
#[derive(Debug)]struct Admission;
impl RuntimeAllocationAdmission for Admission{
 fn try_reserve(&self,r:RuntimeAllocationRequest)->Result<Arc<dyn RuntimeAllocationReceipt>,ResourceLayoutError>{
  let task=r.kind=="tokio_async_task";
  if task { EXPECTED_CELL_SIZE.store(r.bytes,Ordering::SeqCst);RECEIPT_LIVE.store(true,Ordering::SeqCst); }
  Ok(Arc::new(Receipt{bytes:r.bytes,task}))
 }
 fn record_failure(&self,e:ResourceLayoutError){panic!("unexpected denial: {e}");}
}
#[test]fn native_cell_receipt_remains_live_during_allocator_deallocation(){
 let profile=LocalRuntimeProfile{worker_threads:1,blocking_threads:1,blocking_queue:4,thread_stack_bytes:2<<20,async_tasks:4,blocking_tasks:16};
 let runtime=profile.build(Arc::new(Admission),||{},||{}).unwrap();
 let (send,receive)=std::sync::mpsc::channel();let mut send=Some(send);
 let task=runtime.spawn(std::future::poll_fn(move|cx|{if let Some(send)=send.take(){send.send(cx.waker().clone()).unwrap();}std::task::Poll::<()>::Pending}));
 let waker=receive.recv().unwrap();assert!(!CELL_POINTER.load(Ordering::SeqCst).is_null());
 task.abort();assert!(runtime.block_on(task).unwrap_err().is_cancelled());drop(runtime);
 assert!(!CELL_DEALLOCATED.load(Ordering::SeqCst));assert!(RECEIPT_LIVE.load(Ordering::SeqCst));
 drop(waker);
 assert!(CELL_DEALLOCATED.load(Ordering::SeqCst));assert!(LIVE_AT_DEALLOC.load(Ordering::SeqCst));assert!(!RECEIPT_LIVE.load(Ordering::SeqCst));
}
