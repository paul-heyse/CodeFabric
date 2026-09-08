//! Admission around the original object_store0.13.2 BufWriter. Transaction,
//! multipart ordering, conditional put and native cleanup remain upstream-owned.
use bytes::Bytes;
use delta_kernel::resource::{AllocationRequest, NativeResourceScope, ResourceExhausted};
use futures::future::BoxFuture;
use object_store::{
    PutOptions, PutPayload, PutPayloadMut, UploadPart, WriteMultipart, buffered::BufWriter,
};
use parquet::{
    arrow::async_writer::AsyncFileWriter,
    errors::{ParquetError, Result as ParquetResult},
};
use std::{
    fmt,
    mem::{align_of, size_of, size_of_val},
    sync::Arc,
};
use tokio::io::AsyncWriteExt;

use crate::DeltaResult;

fn overflow() -> delta_kernel::Error {
    ResourceExhausted {
        kind: "native_buffered_writer_layout",
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}
fn add(a: usize, b: usize) -> delta_kernel::DeltaResult<usize> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> delta_kernel::DeltaResult<usize> {
    a.checked_mul(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(overflow)
}
fn fields(fields: &[(usize, usize)]) -> delta_kernel::DeltaResult<usize> {
    let alignment = fields
        .iter()
        .map(|(_, alignment)| *alignment)
        .max()
        .unwrap_or(1);
    fields.iter().try_fold(0, |sum, (size, _)| {
        add(sum, add(*size, alignment - 1)? & !(alignment - 1))
    })
}
fn pressure(error: ResourceExhausted) -> ParquetError {
    ParquetError::ResourceExhausted(parquet::resource::ResourceExhausted {
        kind: error.kind,
        requested: error.requested,
        limit: error.limit,
    })
}
fn kernel_error(error: delta_kernel::Error) -> ParquetError {
    match error {
        delta_kernel::Error::ResourceExhausted(error) => pressure(error),
        // Scope admission returns typed resource errors. Preserve any future
        // non-resource extension through the original native error semantics.
        error => ParquetError::External(Box::new(error)),
    }
}

#[derive(Debug)]
struct OwnedWriteError<E> {
    error: E,
    _owner: Option<Arc<NativeResourceScope>>,
}
impl<E: fmt::Display> fmt::Display for OwnedWriteError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}
impl<E: std::error::Error + 'static> std::error::Error for OwnedWriteError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// A single mutable native BufWriter; original buffers drop before scope owner.
#[derive(Debug)]
pub(crate) struct AdmittedBufWriter {
    inner: BufWriter,
    chunk_size: usize,
    calls: usize,
    finished: bool,
    scope: Option<Arc<NativeResourceScope>>,
}
impl AdmittedBufWriter {
    pub(crate) fn try_new(inner: BufWriter, chunk_size: usize) -> DeltaResult<Self> {
        if chunk_size == 0 {
            return Err(delta_kernel::Error::from(ResourceExhausted {
                kind: "native_upload_chunk_size",
                requested: 1,
                limit: 0,
            })
            .into());
        }
        let mut value = Self {
            inner,
            chunk_size,
            calls: 0,
            finished: false,
            scope: delta_kernel::resource::current_resource_scope(),
        };
        if let Some(scope) = value.scope.clone() {
            // Constructing these async futures is allocation-free: neither body
            // runs. Their exact compiler layouts bound the one exclusive
            // AsyncFileWriter BoxFuture held by &mut self at any instant.
            let write_size = {
                let future = value.write_inner(Bytes::new());
                size_of_val(&future)
            };
            let complete_size = {
                let future = value.complete_inner();
                size_of_val(&future)
            };
            let tasks = delta_kernel_default_engine::resource::external_join_set_bytes::<UploadPart>(
                0, true,
            )?;
            // Native BufWriter::poll_shutdown owns exactly one Flush BoxFuture.
            // Sum every captured/local field in both native branches plus its
            // nested multipart finish future and dynamic store call. Padding
            // each field to maximum alignment bounds repr(Rust) field order.
            let flush = fields(&[
                (
                    size_of::<Arc<dyn object_store::ObjectStore>>(),
                    align_of::<Arc<dyn object_store::ObjectStore>>(),
                ),
                (
                    size_of::<object_store::path::Path>(),
                    align_of::<object_store::path::Path>(),
                ),
                (size_of::<PutPayloadMut>(), align_of::<PutPayloadMut>()),
                (size_of::<PutPayload>(), align_of::<PutPayload>()),
                (size_of::<PutOptions>(), align_of::<PutOptions>()),
                (size_of::<WriteMultipart>(), align_of::<WriteMultipart>()),
                (size_of::<WriteMultipart>(), align_of::<WriteMultipart>()),
                (
                    size_of::<BoxFuture<'static, object_store::Result<object_store::PutResult>>>(),
                    align_of::<BoxFuture<'static, object_store::Result<object_store::PutResult>>>(),
                ),
                (
                    size_of::<BoxFuture<'static, object_store::Result<()>>>(),
                    align_of::<BoxFuture<'static, object_store::Result<()>>>(),
                ),
                // wait_for_capacity poll_fn captures &mut WriteMultipart and
                // maximum; finish retains optional part and results across await.
                (4 * size_of::<usize>(), align_of::<usize>()),
                (
                    size_of::<Option<PutPayload>>(),
                    align_of::<Option<PutPayload>>(),
                ),
                (
                    size_of::<object_store::Result<object_store::PutResult>>(),
                    align_of::<object_store::Result<object_store::PutResult>>(),
                ),
                (
                    size_of::<Result<object_store::Result<()>, tokio::task::JoinError>>(),
                    align_of::<Result<object_store::Result<()>, tokio::task::JoinError>>(),
                ),
                (4 * size_of::<usize>(), align_of::<usize>()), // async discriminants/reference slots
            ])?;
            let initial_descriptors = mul(16, size_of::<Bytes>())?;
            scope.reserve(AllocationRequest {
                kind: "native_buffered_writer_fixed",
                bytes: add(
                    add(write_size.max(complete_size), flush)?,
                    add(tasks, initial_descriptors)?,
                )?,
            })?;
        }
        Ok(value)
    }

    fn check_owner(&self) -> delta_kernel::DeltaResult<()> {
        if let Some(scope) = &self.scope {
            let current = delta_kernel::resource::current_resource_scope();
            if current
                .as_ref()
                .is_none_or(|current| !Arc::ptr_eq(current, scope))
            {
                let error = ResourceExhausted {
                    kind: "native_buffered_writer_foreign_scope",
                    requested: 1,
                    limit: 0,
                };
                scope.record_failure(error);
                return Err(error.into());
            }
            scope.check_available()?;
            if self.finished {
                return Err(ResourceExhausted {
                    kind: "native_buffered_writer_finished",
                    requested: 1,
                    limit: 0,
                }
                .into());
            }
        }
        Ok(())
    }
    fn admit(&mut self, bytes: usize, completing: bool) -> delta_kernel::DeltaResult<()> {
        self.check_owner()?;
        let calls = add(self.calls, 1)?;
        let parts = if completing {
            1
        } else {
            add(bytes / self.chunk_size, 2)?
        };
        if let Some(scope) = &self.scope {
            let tasks = delta_kernel_default_engine::resource::external_join_set_bytes::<UploadPart>(
                parts, false,
            )?;
            // put(Bytes) never calls extend_from_slice. Native payloads retain
            // original Bytes/slices. At most one new input segment plus one
            // fragment per chunk boundary is appended. The initial buffering
            // phase can replay its previous <=calls segments once into the
            // multipart collector, so include them before each possible replay.
            let fragments = add(add(parts, calls)?, 1)?;
            // Each Vec<Bytes> geometrically grows from minimum4. Four full
            // layouts per fragment plus16 slots per new collector dominate all
            // replacements; one further slot pays the Arc<[Bytes]> freeze copy.
            let slots = add(mul(fragments, 5)?, mul(parts, 16)?)?;
            let arrays = add(
                mul(slots, size_of::<Bytes>())?,
                mul(add(parts, 1)?, 2 * size_of::<usize>())?,
            )?;
            // bytes1.12.1 may allocate Shared backing metadata on first slice of
            // a Vec-backed Bytes; the original payload allocation is separate.
            let shared = mul(fragments, 4 * size_of::<usize>())?;
            let error = add(
                size_of::<OwnedWriteError<object_store::Error>>(),
                add(
                    size_of::<OwnedWriteError<std::io::Error>>(),
                    size_of::<object_store::Error>(),
                )?,
            )?;
            scope.reserve(AllocationRequest {
                kind: "native_buffered_writer_parts",
                bytes: add(add(tasks, arrays)?, add(shared, error)?)?,
            })?;
        }
        self.calls = calls;
        Ok(())
    }

    async fn write_inner(&mut self, bytes: Bytes) -> ParquetResult<()> {
        self.admit(bytes.len(), false).map_err(kernel_error)?;
        let owner = self.scope.clone();
        self.inner.put(bytes).await.map_err(|error| {
            ParquetError::External(Box::new(OwnedWriteError {
                error,
                _owner: owner,
            }))
        })
    }
    async fn complete_inner(&mut self) -> ParquetResult<()> {
        self.admit(0, true).map_err(kernel_error)?;
        let owner = self.scope.clone();
        self.inner.shutdown().await.map_err(|error| {
            ParquetError::External(Box::new(OwnedWriteError {
                error,
                _owner: owner,
            }))
        })?;
        self.finished = true;
        Ok(())
    }
}
impl AsyncFileWriter for AdmittedBufWriter {
    fn try_write(&mut self, bytes: Bytes) -> ParquetResult<BoxFuture<'_, ParquetResult<()>>> {
        self.check_owner().map_err(kernel_error)?;
        Ok(self.write(bytes))
    }
    fn try_complete(&mut self) -> ParquetResult<BoxFuture<'_, ParquetResult<()>>> {
        self.check_owner().map_err(kernel_error)?;
        Ok(self.complete())
    }
    fn write(&mut self, bytes: Bytes) -> BoxFuture<'_, ParquetResult<()>> {
        Box::pin(self.write_inner(bytes))
    }
    fn complete(&mut self) -> BoxFuture<'_, ParquetResult<()>> {
        Box::pin(self.complete_inner())
    }
}
