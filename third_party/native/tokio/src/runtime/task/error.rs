use std::any::Any;
use std::fmt;
use std::io;

use super::Id;
use crate::util::SyncWrapper;
cfg_rt! {
    /// Task failed to execute to completion.
    pub struct JoinError {
        repr: Repr,
        id: Id,
    }
}

enum Repr {
    #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
    Resource(crate::runtime::resource::ResourceLayoutError),
    Cancelled,
    Panic(SyncWrapper<Box<dyn Any + Send + 'static>>),
}

impl JoinError {
    #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
    pub(crate) fn resource_denied(
        id: Id,
        error: crate::runtime::resource::ResourceLayoutError,
    ) -> Self {
        Self {
            repr: Repr::Resource(error),
            id,
        }
    }
    /// Returns original allocation pressure when native task admission refused work.
    #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
    pub fn resource_error(&self) -> Option<&crate::runtime::resource::ResourceLayoutError> {
        match &self.repr {
            Repr::Resource(e) => Some(e),
            _ => None,
        }
    }

    pub(crate) fn cancelled(id: Id) -> JoinError {
        JoinError {
            repr: Repr::Cancelled,
            id,
        }
    }

    pub(crate) fn panic(id: Id, err: Box<dyn Any + Send + 'static>) -> JoinError {
        JoinError {
            repr: Repr::Panic(SyncWrapper::new(err)),
            id,
        }
    }

    /// Returns true if the error was caused by the task being cancelled.
    ///
    /// See [the module level docs] for more information on cancellation.
    ///
    /// [the module level docs]: crate::task#cancellation
    pub fn is_cancelled(&self) -> bool {
        matches!(&self.repr, Repr::Cancelled)
    }

    /// Returns true if the error was caused by the task panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(not(target_family = "wasm"))]
    /// # {
    /// use std::panic;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let err = tokio::spawn(async {
    ///         panic!("boom");
    ///     }).await.unwrap_err();
    ///
    ///     assert!(err.is_panic());
    /// }
    /// # }
    /// ```
    pub fn is_panic(&self) -> bool {
        matches!(&self.repr, Repr::Panic(_))
    }

    /// Consumes the join error, returning the object with which the task panicked.
    ///
    /// # Panics
    ///
    /// `into_panic()` panics if the `Error` does not represent the underlying
    /// task terminating with a panic. Use `is_panic` to check the error reason
    /// or `try_into_panic` for a variant that does not panic.
    ///
    /// # Examples
    ///
    /// ```should_panic,ignore-wasm
    /// use std::panic;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let err = tokio::spawn(async {
    ///         panic!("boom");
    ///     }).await.unwrap_err();
    ///
    ///     if err.is_panic() {
    ///         // Resume the panic on the main task
    ///         panic::resume_unwind(err.into_panic());
    ///     }
    /// }
    /// ```
    #[track_caller]
    pub fn into_panic(self) -> Box<dyn Any + Send + 'static> {
        self.try_into_panic()
            .expect("`JoinError` reason is not a panic.")
    }

    /// Consumes the join error, returning the object with which the task
    /// panicked if the task terminated due to a panic. Otherwise, `self` is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```should_panic,ignore-wasm
    /// use std::panic;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let err = tokio::spawn(async {
    ///         panic!("boom");
    ///     }).await.unwrap_err();
    ///
    ///     if let Ok(reason) = err.try_into_panic() {
    ///         // Resume the panic on the main task
    ///         panic::resume_unwind(reason);
    ///     }
    /// }
    /// ```
    pub fn try_into_panic(self) -> Result<Box<dyn Any + Send + 'static>, JoinError> {
        match self.repr {
            Repr::Panic(p) => Ok(p.into_inner()),
            _ => Err(self),
        }
    }

    /// Returns a [task ID] that identifies the task which errored relative to
    /// other currently spawned tasks.
    ///
    /// [task ID]: crate::task::Id
    pub fn id(&self) -> Id {
        self.id
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
            Repr::Resource(error) => error.fmt(fmt),
            Repr::Cancelled => write!(fmt, "task {} was cancelled", self.id),
            Repr::Panic(p) => match panic_payload_as_str(p) {
                Some(panic_str) => {
                    write!(
                        fmt,
                        "task {} panicked with message {:?}",
                        self.id, panic_str
                    )
                }
                None => {
                    write!(fmt, "task {} panicked", self.id)
                }
            },
        }
    }
}

impl fmt::Debug for JoinError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
            Repr::Resource(error) => error.fmt(fmt),
            Repr::Cancelled => write!(fmt, "JoinError::Cancelled({:?})", self.id),
            Repr::Panic(p) => match panic_payload_as_str(p) {
                Some(panic_str) => {
                    write!(fmt, "JoinError::Panic({:?}, {:?}, ...)", self.id, panic_str)
                }
                None => write!(fmt, "JoinError::Panic({:?}, ...)", self.id),
            },
        }
    }
}

impl std::error::Error for JoinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
        if let Repr::Resource(error) = &self.repr {
            return Some(error);
        }
        None
    }
}

impl From<JoinError> for io::Error {
    fn from(src: JoinError) -> io::Error {
        #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
        if let Repr::Resource(_) = &src.repr {
            return io::ErrorKind::OutOfMemory.into();
        }
        io::Error::new(
            io::ErrorKind::Other,
            match src.repr {
                #[cfg(all(feature = "rt-multi-thread", feature = "time"))]
                Repr::Resource(_) => unreachable!("resource denial handled without allocation"),
                Repr::Cancelled => "task was cancelled",
                Repr::Panic(_) => "task panicked",
            },
        )
    }
}

fn panic_payload_as_str(payload: &SyncWrapper<Box<dyn Any + Send>>) -> Option<&str> {
    // Panic payloads are almost always `String` (if invoked with formatting arguments)
    // or `&'static str` (if invoked with a string literal).
    //
    // Non-string panic payloads have niche use-cases,
    // so we don't really need to worry about those.
    if let Some(s) = payload.downcast_ref_sync::<String>() {
        return Some(s);
    }

    if let Some(s) = payload.downcast_ref_sync::<&'static str>() {
        return Some(s);
    }

    None
}
