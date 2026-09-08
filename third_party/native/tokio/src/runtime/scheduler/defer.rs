use std::cell::RefCell;
use std::task::Waker;

// The selected owned profile has no heap allocation per Defer/worker handoff.
// Timer and cooperative-yield semantics permit immediate rescheduling when full.
const INLINE_DEFERRED: usize = 32;
enum Deferred {
    Native(Vec<Waker>),
    Inline { values: [Option<Waker>; INLINE_DEFERRED], len: usize },
}
pub(crate) struct Defer { deferred: RefCell<Deferred> }
impl Defer {
    pub(crate) fn new() -> Defer {
        #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
        if crate::runtime::resource::current().is_some() {
            return Self { deferred: RefCell::new(Deferred::Inline { values: std::array::from_fn(|_| None), len: 0 }) };
        }
        Self { deferred: RefCell::new(Deferred::Native(Vec::new())) }
    }
    pub(crate) fn defer(&self, waker: &Waker) {
        let mut deferred = self.deferred.borrow_mut();
        match &mut *deferred {
            Deferred::Native(values) => {
                if values.last().is_some_and(|last| last.will_wake(waker)) { return; }
                values.push(waker.clone());
            }
            Deferred::Inline {values,len} => {
                if *len > 0 && values[*len-1].as_ref().is_some_and(|last| last.will_wake(waker)) { return; }
                if *len == values.len() { drop(deferred); waker.wake_by_ref(); return; }
                values[*len] = Some(waker.clone());
                *len += 1;
            }
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        match &*self.deferred.borrow() { Deferred::Native(v)=>v.is_empty(),Deferred::Inline{len,..}=>*len==0 }
    }
    pub(crate) fn wake(&self) {
        loop {
            let value = match &mut *self.deferred.borrow_mut() {
                Deferred::Native(v)=>v.pop(),
                Deferred::Inline{values,len}=>if *len == 0 {None} else {*len-=1;values[*len].take()},
            };
            match value {Some(waker)=>waker.wake(),None=>break}
        }
    }
    #[cfg(feature = "taskdump")]
    pub(crate) fn take_deferred(&self) -> Vec<Waker> {
        match &mut *self.deferred.borrow_mut() {
            Deferred::Native(values) => std::mem::take(values),
            // The owned profile rejects taskdump before runtime construction.
            Deferred::Inline { .. } => unreachable!("owned local taskdump is disabled"),
        }
    }
}
