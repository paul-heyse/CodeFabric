use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}};
use tokio::runtime::resource::*;
#[derive(Debug,Default)]struct Accounting{live:Arc<AtomicUsize>,requests:Mutex<Vec<usize>>,failures:Mutex<Vec<ResourceLayoutError>>,deny:bool,deny_tasks:AtomicBool}
#[derive(Debug)]struct Receipt{bytes:usize,live:Arc<AtomicUsize>}
impl RuntimeAllocationReceipt for Receipt{fn bytes(&self)->usize{self.bytes}}
impl Drop for Receipt{fn drop(&mut self){self.live.fetch_sub(self.bytes,Ordering::SeqCst);}}
impl RuntimeAllocationAdmission for Accounting {
 fn try_reserve(&self,request:RuntimeAllocationRequest)->Result<Arc<dyn RuntimeAllocationReceipt>,ResourceLayoutError>{let bytes=request.bytes;self.requests.lock().unwrap().push(bytes);if self.deny || (request.kind!="tokio_local_runtime" && self.deny_tasks.load(Ordering::SeqCst)){return Err(ResourceLayoutError{kind:"test_runtime_capacity",requested:bytes,limit:0})}self.live.fetch_add(bytes,Ordering::SeqCst);Ok(Arc::new(Receipt{bytes,live:self.live.clone()}))}
 fn record_failure(&self,error:ResourceLayoutError){self.failures.lock().unwrap().push(error);}
}
fn profile()->LocalRuntimeProfile{LocalRuntimeProfile{worker_threads:1,blocking_threads:2,blocking_queue:4,async_tasks:128,blocking_tasks:128,thread_stack_bytes:2<<20}}
#[test]fn denied_profile_constructs_no_worker_and_latches_pressure(){
 let accounting=Arc::new(Accounting{deny:true,..Default::default()});let starts=Arc::new(AtomicUsize::new(0));let flag=starts.clone();
 let error=profile().build(accounting.clone(),move||{flag.fetch_add(1,Ordering::SeqCst);},||{}).unwrap_err();
 assert!(matches!(error,LocalRuntimeBuildError::Resource(_)));assert_eq!(starts.load(Ordering::SeqCst),0);assert_eq!(accounting.failures.lock().unwrap().len(),1);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn original_runtime_handles_keep_receipt_after_join(){
 let accounting=Arc::new(Accounting::default());let started=Arc::new(AtomicUsize::new(0));let stopped=Arc::new(AtomicUsize::new(0));let a=started.clone();let b=stopped.clone();
 let runtime=profile().build(accounting.clone(),move||{a.fetch_add(1,Ordering::SeqCst);},move||{b.fetch_add(1,Ordering::SeqCst);}).unwrap();
 let handle=runtime.handle().clone();runtime.block_on(async{tokio::time::sleep(std::time::Duration::from_millis(1)).await;assert_eq!(tokio::task::spawn_blocking(||17).await.unwrap(),17);});
 drop(runtime);assert_eq!(started.load(Ordering::SeqCst),stopped.load(Ordering::SeqCst));assert!(accounting.live.load(Ordering::SeqCst)>0);drop(handle);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn native_blocking_queue_capacity_rejects_before_enqueue_and_input_poll(){
 let accounting=Arc::new(Accounting::default());let mut p=profile();p.blocking_threads=1;p.blocking_queue=1;
 let runtime=p.build(accounting.clone(),||{},||{}).unwrap();runtime.block_on(async{runtime.spawn(async{}).await.unwrap();});let (started_tx,started_rx)=std::sync::mpsc::channel();let (release_tx,release_rx)=std::sync::mpsc::channel();
 let running=runtime.spawn_blocking(move||{started_tx.send(()).unwrap();release_rx.recv().unwrap();});started_rx.recv().unwrap();
 let pending=runtime.spawn_blocking(||42);let input=Arc::new(AtomicUsize::new(0));let mark=input.clone();let denied=runtime.spawn_blocking(move||{mark.fetch_add(1,Ordering::SeqCst);});
 assert_eq!(accounting.failures.lock().unwrap()[0].kind,"tokio_blocking_queue");assert_eq!(input.load(Ordering::SeqCst),0);
 release_tx.send(()).unwrap();runtime.block_on(async{running.await.unwrap();assert_eq!(pending.await.unwrap(),42);assert!(denied.await.unwrap_err().is_cancelled());});drop(runtime);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn actual_layout_scales_per_worker_and_refuses_overflow_before_admission(){
 let p=profile();let small=p.layout::<fn(),fn()>().unwrap();let mut large=p;large.worker_threads=2;let large=large.layout::<fn(),fn()>().unwrap();assert!(large.scheduler_bytes>small.scheduler_bytes);assert!(small.driver_bytes>0);assert!(small.thread_park_bytes>0);assert!(small.tokio_tls_bytes_per_thread>0);
 let accounting=Arc::new(Accounting::default());let mut invalid=p;invalid.blocking_threads=usize::MAX;assert!(invalid.build(accounting.clone(),||{},||{}).is_err());assert!(accounting.requests.lock().unwrap().is_empty());
}
#[test]fn local_runtime_rejects_network_driver_selection(){
 let accounting=Arc::new(Accounting::default());let runtime=profile().build(accounting.clone(),||{},||{}).unwrap();
 let network=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||runtime.block_on(async{let socket=std::net::TcpListener::bind("127.0.0.1:0").unwrap();socket.set_nonblocking(true).unwrap();tokio::net::TcpListener::from_std(socket)})));
 assert!(network.is_err());drop(runtime);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}

#[test]fn cumulative_async_ceiling_is_not_reused_after_join(){
 let accounting=Arc::new(Accounting::default());let mut p=profile();p.async_tasks=2;let runtime=p.build(accounting.clone(),||{},||{}).unwrap();
 for n in 0..2 {assert_eq!(runtime.block_on(runtime.spawn(async move{n})).unwrap(),n);}
 let never=Arc::new(AtomicUsize::new(0));let mark=never.clone();let task=runtime.spawn(async move{mark.fetch_add(1,Ordering::SeqCst);});
 assert!(task.is_finished());let abort=task.abort_handle();assert_eq!(task.id(),abort.id());abort.abort();assert!(abort.clone().is_finished());
 let error=runtime.block_on(task).unwrap_err();assert_eq!(error.resource_error().unwrap().kind,"tokio_async_task");assert!(!error.is_panic());assert!(!error.is_cancelled());assert_eq!(never.load(Ordering::SeqCst),0);drop(abort);drop(runtime);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn actual_native_waker_keeps_task_receipt_after_cancel_and_runtime_join(){
 let accounting=Arc::new(Accounting::default());let runtime=profile().build(accounting.clone(),||{},||{}).unwrap();let (send,receive)=std::sync::mpsc::channel();let mut send=Some(send);
 let task=runtime.spawn(std::future::poll_fn(move|cx|{if let Some(send)=send.take(){send.send(cx.waker().clone()).unwrap();}std::task::Poll::<()>::Pending}));let waker=receive.recv().unwrap();task.abort();assert!(runtime.block_on(task).unwrap_err().is_cancelled());drop(runtime);
 assert!(accounting.live.load(Ordering::SeqCst)>0);drop(waker);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn denied_native_worker_launch_closes_partial_original_runtime(){
 let accounting=Arc::new(Accounting::default());accounting.deny_tasks.store(true,Ordering::SeqCst);let starts=Arc::new(AtomicUsize::new(0));let start=starts.clone();
 let error=profile().build(accounting.clone(),move||{start.fetch_add(1,Ordering::SeqCst);},||{}).unwrap_err();assert!(matches!(error,LocalRuntimeBuildError::Resource(_)));assert_eq!(starts.load(Ordering::SeqCst),0);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn failed_blocking_admission_precedes_actual_tokio_filesystem_work(){
 let path=std::env::temp_dir().join(format!("tokio-owned-deny-{}",std::process::id()));let _=std::fs::remove_file(&path);
 let accounting=Arc::new(Accounting::default());let runtime=profile().build(accounting.clone(),||{},||{}).unwrap();accounting.deny_tasks.store(true,Ordering::SeqCst);
 let error=runtime.block_on(tokio::fs::write(&path,b"must not be written")).unwrap_err();assert_eq!(error.kind(),std::io::ErrorKind::OutOfMemory);assert!(!path.exists());drop(runtime);assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}

#[derive(Debug)]struct LaunchDenial{inner:Accounting,allowed:usize,requests:AtomicUsize,started:Arc<AtomicUsize>}
impl RuntimeAllocationAdmission for LaunchDenial{
 fn try_reserve(&self,r:RuntimeAllocationRequest)->Result<Arc<dyn RuntimeAllocationReceipt>,ResourceLayoutError>{
  if r.kind=="tokio_blocking_task" && self.requests.fetch_add(1,Ordering::SeqCst)==self.allowed {
   while self.started.load(Ordering::SeqCst)<self.allowed {std::thread::yield_now();}
   return Err(ResourceLayoutError{kind:"test_nth_native_launch",requested:self.allowed+1,limit:self.allowed});
  }
  self.inner.try_reserve(r)
 }
 fn record_failure(&self,e:ResourceLayoutError){self.inner.record_failure(e);}
}
#[test]fn each_partial_scheduler_launch_joins_started_workers_and_releases_original_owners(){
 for allowed in 0..3 {
  let started=Arc::new(AtomicUsize::new(0));let stopped=Arc::new(AtomicUsize::new(0));
  let admission=Arc::new(LaunchDenial{inner:Accounting::default(),allowed,requests:AtomicUsize::new(0),started:started.clone()});
  let a=started.clone();let b=stopped.clone();let mut p=profile();p.worker_threads=3;
  let result=p.build(admission.clone(),move||{a.fetch_add(1,Ordering::SeqCst);},move||{b.fetch_add(1,Ordering::SeqCst);});
  assert!(matches!(result,Err(LocalRuntimeBuildError::Resource(ResourceLayoutError{kind:"test_nth_native_launch",..}))));
  assert_eq!(started.load(Ordering::SeqCst),allowed);assert_eq!(stopped.load(Ordering::SeqCst),allowed);
  assert_eq!(admission.inner.live.load(Ordering::SeqCst),0,"native root or task receipt escaped failed launch {allowed}");
 }
}
#[test]fn mandatory_file_write_pressure_is_allocation_free_error_before_native_write(){
 use tokio::io::AsyncWriteExt;
 let path=std::env::temp_dir().join(format!("tokio-owned-mandatory-{}",std::process::id()));
 let accounting=Arc::new(Accounting::default());let runtime=profile().build(accounting.clone(),||{},||{}).unwrap();
 let mut file=runtime.block_on(tokio::fs::File::create(&path)).unwrap();accounting.deny_tasks.store(true,Ordering::SeqCst);
 let error=runtime.block_on(file.write_all(b"must not reach native write")).unwrap_err();assert_eq!(error.kind(),std::io::ErrorKind::OutOfMemory);
 drop(file);drop(runtime);assert!(std::fs::read(&path).unwrap().is_empty());std::fs::remove_file(path).unwrap();
 assert_eq!(accounting.live.load(Ordering::SeqCst),0);
}
#[test]fn failed_worker_handoff_never_enters_blocking_user_closure(){
 let accounting=Arc::new(Accounting::default());let runtime=profile().build(accounting.clone(),||{},||{}).unwrap();let entry=Arc::new(AtomicUsize::new(0));let marker=entry.clone();
 let result=runtime.block_on(runtime.spawn(async move{
  accounting.deny_tasks.store(true,Ordering::SeqCst);
  tokio::runtime::resource::try_block_in_place(||{marker.fetch_add(1,Ordering::SeqCst);})
 })).unwrap();
 assert!(result.is_err());assert_eq!(entry.load(Ordering::SeqCst),0);
}
