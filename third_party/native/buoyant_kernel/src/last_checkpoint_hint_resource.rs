//! Original checkpoint-hint admission, distinct from its optional-hint semantics.
use std::mem::size_of;
use std::sync::Arc;

use super::{HintAction, LastCheckpointHint, LastCheckpointV2, Sidecar};
use crate::crc::resource::{add, mul};
use crate::resource::{
    AllocationReceipt, AllocationRequest, JsonResourceLimits, JsonShape, NativeResourceScope,
    ResourceExhausted,
};
use crate::{DeltaResult, Error};

#[derive(Debug, Default)]
pub(super) struct HintResourceOwner {
    scope: Option<Arc<NativeResourceScope>>,
    shape: Option<JsonShape>,
    retained: usize,
}
// A native deep Clone makes fresh backing; it cannot inherit admission.
impl Clone for HintResourceOwner {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PartialEq for HintResourceOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for HintResourceOwner {}

fn decode_bound(shape: JsonShape) -> DeltaResult<usize> {
    // CRC's typed-action geometry covers Metadata/Protocol/Txn/DomainMetadata,
    // feature Content and their maps/strings/diagnostics. The embedded checkpoint
    // schema additionally executes native StructType serde (including recursive
    // DataType Value/Content conversions), whose entire source bound is added.
    let actions = crate::crc::resource::decode_bytes(shape)?;
    let schema = crate::schema::StructType::container_decode_bound(shape)?;
    // Hint-specific vectors move complete enum/sidecar values. RawVec capacity
    // growth sums to <4*(items+minimums); JSON containers each allow 8 minimum
    // slots. Its tags/sidecar/checkpoint-action String maps obey the same pinned
    // hashbrown growth used in CRC, with their separate bucket/control layouts.
    let elements = add(shape.tokens, mul(shape.containers, 8)?)?;
    let vectors = mul(
        mul(elements, 4)?,
        add(size_of::<HintAction>(), size_of::<Sidecar>())?,
    )?;
    let maps = add(
        mul(mul(elements, 8)?, add(size_of::<(String, String)>(), 1)?)?,
        mul(add(shape.tokens, shape.containers)?, 32)?,
    )?;
    // Hint-only path/checksum/tags strings and serde scratch: decoding never
    // expands UTF-8, and cumulative geometric reallocations are <4*(bytes+8).
    let strings = mul(add(shape.bytes, 8)?, 4)?;
    [
        actions,
        schema,
        vectors,
        maps,
        strings,
        size_of::<LastCheckpointHint>(),
        size_of::<LastCheckpointV2>(),
        size_of::<HintDecodeError>(),
        4 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0, add)
}

#[derive(Debug)]
struct HintDecodeError {
    error: serde_json::Error,
    _scope: Arc<NativeResourceScope>,
    _scratch: Arc<dyn AllocationReceipt>,
}
impl std::fmt::Display for HintDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for HintDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl LastCheckpointHint {
    /// Decode the original native hint after finite admission. Threshold drops
    /// remain optimization semantics; they are never used as decode limits.
    pub fn from_bytes_with_resources(
        bytes: &[u8],
        scope: Arc<NativeResourceScope>,
        limits: JsonResourceLimits,
    ) -> DeltaResult<Self> {
        crate::resource::with_resource_scope(scope.clone(), limits, || {
            let limits = crate::resource::current_json_resource_limits().unwrap_or(limits);
            let shape = limits.inspect(bytes)?;
            let retained = decode_bound(shape)?;
            scope.reserve(AllocationRequest {
                kind: "native_checkpoint_hint_retained",
                bytes: retained,
            })?;
            let scratch = scope.reserve_transient(AllocationRequest {
                kind: "native_checkpoint_hint_decode",
                bytes: retained,
            })?;
            let mut hint: Self = serde_json::from_slice(bytes).map_err(|error| {
                Error::generic_err(HintDecodeError {
                    error,
                    _scope: scope.clone(),
                    _scratch: scratch.clone(),
                })
            })?;
            hint.attach_original_children(scope.clone(), shape)?;
            hint.resource_owner = HintResourceOwner {
                scope: Some(scope),
                shape: Some(shape),
                retained,
            };
            Ok(hint.drop_oversized_fields())
        })
    }

    fn attach_original_children(
        &mut self,
        scope: Arc<NativeResourceScope>,
        shape: JsonShape,
    ) -> DeltaResult<()> {
        if let Some(schema) = &mut self.checkpoint_schema {
            // Serde has just created this Arc; no cloned or reparsed schema is
            // substituted for its original allocation.
            let schema = Arc::get_mut(schema).ok_or(ResourceExhausted {
                kind: "native_checkpoint_hint_shared_decode_schema",
                requested: 1,
                limit: 0,
            })?;
            schema.attach_container_decode_owner(scope.clone(), shape)?;
        }
        if let Some(sidecars) = self
            .v2_checkpoint
            .as_mut()
            .and_then(|v2| v2.sidecar_files.as_mut())
        {
            for sidecar in sidecars {
                sidecar.attach_admitted_owner(Some(scope.clone()));
            }
        }
        if let Some(actions) = self
            .v2_checkpoint
            .as_mut()
            .and_then(|v2| v2.non_file_actions.as_mut())
        {
            for action in actions {
                match action {
                    HintAction::Metadata(metadata) => {
                        metadata.attach_admitted_owner(Some(scope.clone()))
                    }
                    HintAction::Protocol(protocol) => {
                        protocol.attach_admitted_owner(Some(scope.clone()))
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.resource_owner.scope.as_ref()
    }

    /// Borrow the actual schema allocation decoded with this original hint.
    pub fn native_checkpoint_schema(&self) -> Option<&crate::schema::SchemaRef> {
        self.checkpoint_schema.as_ref()
    }

    /// Borrow original typed hint actions without materializing them again.
    pub fn native_non_file_actions(&self) -> Option<&[HintAction]> {
        self.v2_checkpoint
            .as_ref()
            .and_then(|v2| v2.non_file_actions.as_deref())
    }

    /// Borrow the original sidecar actions and their retained decode owner.
    pub fn native_sidecars(&self) -> Option<&[Sidecar]> {
        self.v2_checkpoint
            .as_ref()
            .and_then(|v2| v2.sidecar_files.as_deref())
    }

    /// Explicit deep copy for consumers requiring mutable hint backing. Normal
    /// LogSegment copies share Arc<LastCheckpointHint> and need no deep copy.
    pub fn try_clone_admitted(&self) -> DeltaResult<Self> {
        let scope = crate::crc::resource::select_scope(None, self.resource_owner.scope.clone())?;
        let Some(scope) = scope else {
            return Ok(self.clone());
        };
        let shape = self.resource_owner.shape.ok_or(ResourceExhausted {
            kind: "native_checkpoint_hint_unadmitted_copy_source",
            requested: 1,
            limit: 0,
        })?;
        scope.reserve(AllocationRequest {
            kind: "native_checkpoint_hint_copy",
            bytes: self.resource_owner.retained,
        })?;
        let mut copy = self.clone();
        // Native Clone shares the checkpoint_schema Arc. It retains its actual
        // original owner; action copies have distinct backing admitted above.
        if let Some(sidecars) = copy
            .v2_checkpoint
            .as_mut()
            .and_then(|v2| v2.sidecar_files.as_mut())
        {
            for sidecar in sidecars {
                sidecar.attach_admitted_owner(Some(scope.clone()));
            }
        }
        if let Some(actions) = copy
            .v2_checkpoint
            .as_mut()
            .and_then(|v2| v2.non_file_actions.as_mut())
        {
            for action in actions {
                match action {
                    HintAction::Metadata(metadata) => {
                        metadata.attach_admitted_owner(Some(scope.clone()))
                    }
                    HintAction::Protocol(protocol) => {
                        protocol.attach_admitted_owner(Some(scope.clone()))
                    }
                    _ => {}
                }
            }
        }
        copy.resource_owner = HintResourceOwner {
            scope: Some(scope),
            shape: Some(shape),
            retained: self.resource_owner.retained,
        };
        Ok(copy)
    }
}
