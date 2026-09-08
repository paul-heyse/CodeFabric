//! Shared storage ownership survives dropped seals and uncertain storage operations.
//!
//! The builder is retained by the daemon/registry. Its exact-path ledger, not any particular
//! publication handle, owns stored bytes until deletion is confirmed. Recovery uses the same
//! ledger and reacquires exact object sizes before admitting an object from durable intent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;

use super::{
    ObjectPath, ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation,
    ResultObjectSink, StreamedResultPackageError, StreamedResultPackageLimits,
};

#[derive(Debug)]
struct StoredObject {
    byte_length: u64,
    // One page unit means one stored object, including the manifest. This bounds the map even
    // for zero-byte objects. Memory is a conservative path/key/Arc/tree-node envelope.
    _charge: ResourceReservation,
    operation: AsyncMutex<()>,
}

#[derive(Debug)]
pub(super) struct ResultStorageLedger {
    pub(super) limits: StreamedResultPackageLimits,
    sink: Arc<dyn ResultObjectSink>,
    budget: ResourceBudget,
    objects: Mutex<BTreeMap<ObjectPath, Arc<StoredObject>>>,
}

impl ResultStorageLedger {
    pub(super) fn new(
        sink: Arc<dyn ResultObjectSink>,
        budget: ResourceBudget,
        limits: StreamedResultPackageLimits,
    ) -> Self {
        Self {
            limits,
            sink,
            budget,
            objects: Mutex::new(BTreeMap::new()),
        }
    }

    fn get(&self, path: &ObjectPath) -> Option<Arc<StoredObject>> {
        self.objects
            .lock()
            .expect("storage ledger lock")
            .get(path)
            .cloned()
    }

    fn admit(
        &self,
        path: &ObjectPath,
        byte_length: u64,
        class: ResourceClass,
    ) -> Result<Arc<StoredObject>, StreamedResultPackageError> {
        let mut objects = self.objects.lock().expect("storage ledger lock");
        if let Some(existing) = objects.get(path) {
            if existing.byte_length != byte_length {
                return Err(StreamedResultPackageError::ObjectLength);
            }
            return Ok(Arc::clone(existing));
        }
        let metadata_bytes = (path.as_ref().len() as u64)
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(1024))
            .ok_or(StreamedResultPackageError::CounterOverflow)?;
        let charge = self.budget.try_reserve(
            class,
            ResourceAmounts {
                memory_bytes: metadata_bytes,
                disk_bytes: byte_length,
                retained_bytes: byte_length,
                pages: 1,
                ..ResourceAmounts::default()
            },
        )?;
        let entry = Arc::new(StoredObject {
            byte_length,
            _charge: charge,
            operation: AsyncMutex::new(()),
        });
        objects.insert(path.clone(), Arc::clone(&entry));
        Ok(entry)
    }

    fn is_current(&self, path: &ObjectPath, entry: &Arc<StoredObject>) -> bool {
        self.objects
            .lock()
            .expect("storage ledger lock")
            .get(path)
            .is_some_and(|current| Arc::ptr_eq(current, entry))
    }

    pub(super) async fn create(
        &self,
        path: &ObjectPath,
        bytes: Vec<u8>,
    ) -> Result<(), StreamedResultPackageError> {
        loop {
            let entry = self.admit(path, bytes.len() as u64, ResourceClass::Data)?;
            let _operation = entry.operation.lock().await;
            if !self.is_current(path, &entry) {
                continue;
            }
            // Even a failed put may have persisted the object. Never release its reservation
            // here; the durable intent and a confirmed delete are the recovery authority.
            self.sink.create(path, bytes).await?;
            return Ok(());
        }
    }

    pub(super) async fn adopt(
        &self,
        path: &ObjectPath,
        byte_length: u64,
    ) -> Result<(), StreamedResultPackageError> {
        loop {
            let entry = self.admit(path, byte_length, ResourceClass::Control)?;
            let _operation = entry.operation.lock().await;
            if !self.is_current(path, &entry) {
                continue;
            }
            if self.sink.size(path).await? != byte_length {
                return Err(StreamedResultPackageError::ObjectLength);
            }
            return Ok(());
        }
    }

    pub(super) async fn delete(
        &self,
        path: &ObjectPath,
        maximum_bytes: u64,
    ) -> Result<(), StreamedResultPackageError> {
        loop {
            let entry = if let Some(entry) = self.get(path) {
                entry
            } else {
                let length = match self.sink.size(path).await {
                    Ok(length) => length,
                    Err(object_store::Error::NotFound { .. }) => {
                        // A concurrent create may have acquired ownership during the head.
                        if self.get(path).is_some() {
                            continue;
                        }
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                if length > maximum_bytes {
                    return Err(StreamedResultPackageError::ObjectLength);
                }
                self.admit(path, length, ResourceClass::Control)?
            };
            let _operation = entry.operation.lock().await;
            if !self.is_current(path, &entry) {
                continue;
            }
            match self.sink.delete(path).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                    self.objects
                        .lock()
                        .expect("storage ledger lock")
                        .remove(path);
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}
