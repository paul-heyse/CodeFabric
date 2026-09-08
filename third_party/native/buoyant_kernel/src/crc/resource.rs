//! Native CRC allocation geometry and lifetime ownership.
//!
//! Bounds describe requested allocation layouts, not allocator RSS. The decode bound is
//! specific to CrcRaw and pinned serde 1.0.229 / serde_json 1.0.151 / Rust 1.98.0
//! (std HashMap uses hashbrown 0.17.1). See RESOURCE_BOUND.md for its phase inventory.

use std::collections::HashMap;
use std::mem::{align_of, size_of};
use std::sync::Arc;

use crate::actions::{Add, DomainMetadata, Metadata, Protocol, SetTransaction};
use crate::resource::{AllocationRequest, JsonShape, NativeResourceScope, ResourceExhausted};
use crate::table_features::TableFeature;
use crate::{DeltaResult, Error};

use super::{Crc, CrcDelta, CrcRaw, DomainMetadataState, FileSizeHistogram, SetTransactionState};

/// Compatibility Clone creates a new, unadmitted native allocation. Only explicit
/// fallible copy entry points attach a scope after pre-admitting the entire copy.
#[derive(Debug, Default)]
pub(crate) struct CrcResourceOwner(pub(crate) Option<Arc<NativeResourceScope>>);

impl Clone for CrcResourceOwner {
    fn clone(&self) -> Self {
        Self(None)
    }
}
impl PartialEq for CrcResourceOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for CrcResourceOwner {}

pub(crate) fn overflow() -> Error {
    ResourceExhausted {
        kind: "crc_allocation_layout",
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}

pub(crate) fn add(left: usize, right: usize) -> DeltaResult<usize> {
    left.checked_add(right)
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(overflow)
}
pub(crate) fn mul(left: usize, right: usize) -> DeltaResult<usize> {
    left.checked_mul(right)
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(overflow)
}
fn sum(values: impl IntoIterator<Item = usize>) -> DeltaResult<usize> {
    values.into_iter().try_fold(0, add)
}

/// Upper layout for the pinned std HashMap capacity, including empty control bytes
/// and SIMD group tail. All CRC keys have nonzero layouts larger than a SIMD group.
/// The argument may be current *capacity*, not merely live length, when copying.
pub(crate) fn map_layout<K, V>(capacity: usize) -> DeltaResult<usize> {
    if capacity == 0 {
        return Ok(0);
    }
    let buckets = if capacity < 4 {
        4
    } else if capacity < 8 {
        8
    } else if capacity < 15 {
        16
    } else {
        (mul(capacity, 8)? / 7)
            .checked_next_power_of_two()
            .ok_or_else(overflow)?
    };
    let alignment = align_of::<(K, V)>().max(16);
    let body = mul(buckets, size_of::<(K, V)>())?;
    let control_offset = add(body, alignment - 1)? & !(alignment - 1);
    add(add(control_offset, buckets)?, 16)
}

fn string_map(map: &HashMap<String, String>) -> DeltaResult<usize> {
    map.iter().try_fold(
        map_layout::<String, String>(map.capacity())?,
        |bytes, (key, value)| add(bytes, add(key.len(), value.len())?),
    )
}

pub(crate) fn metadata_bytes(metadata: &Metadata) -> DeltaResult<usize> {
    let mut bytes = sum([
        metadata.id().len(),
        metadata.name().map_or(0, str::len),
        metadata.description().map_or(0, str::len),
        metadata.format_provider().len(),
        metadata.schema_string().len(),
        string_map(metadata.format_options())?,
        string_map(metadata.configuration())?,
        mul(metadata.partition_columns().len(), size_of::<String>())?,
    ])?;
    for column in metadata.partition_columns() {
        bytes = add(bytes, column.len())?;
    }
    Ok(bytes)
}

pub(crate) fn protocol_bytes(protocol: &Protocol) -> DeltaResult<usize> {
    let mut bytes = 0;
    for features in [protocol.reader_features(), protocol.writer_features()]
        .into_iter()
        .flatten()
    {
        bytes = add(bytes, mul(features.len(), size_of::<TableFeature>())?)?;
        for feature in features {
            if let TableFeature::Unknown(value) = feature {
                bytes = add(bytes, value.len())?;
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn histogram_bytes(histogram: &FileSizeHistogram) -> DeltaResult<usize> {
    mul(
        sum([
            histogram.sorted_bin_boundaries.len(),
            histogram.file_counts.len(),
            histogram.total_bytes.len(),
        ])?,
        size_of::<i64>(),
    )
}

fn domain_map(map: &HashMap<String, DomainMetadata>) -> DeltaResult<usize> {
    map.iter().try_fold(
        map_layout::<String, DomainMetadata>(map.capacity())?,
        |bytes, (key, value)| {
            add(
                bytes,
                sum([key.len(), value.domain().len(), value.configuration().len()])?,
            )
        },
    )
}
fn transaction_map(map: &HashMap<String, SetTransaction>) -> DeltaResult<usize> {
    map.iter().try_fold(
        map_layout::<String, SetTransaction>(map.capacity())?,
        |bytes, (key, value)| add(bytes, add(key.len(), value.app_id.len())?),
    )
}
pub(crate) fn domain_state(state: &DomainMetadataState) -> &HashMap<String, DomainMetadata> {
    match state {
        DomainMetadataState::Complete(map) | DomainMetadataState::Partial(map) => map,
    }
}
pub(crate) fn transaction_state(state: &SetTransactionState) -> &HashMap<String, SetTransaction> {
    match state {
        SetTransactionState::Complete(map) | SetTransactionState::Partial(map) => map,
    }
}
fn add_bytes(value: &Add) -> DeltaResult<usize> {
    let mut bytes = sum([
        value.path.len(),
        string_map(&value.partition_values)?,
        value.stats.as_ref().map_or(0, String::len),
        value.clustering_provider.as_ref().map_or(0, String::len),
        value
            .deletion_vector
            .as_ref()
            .map_or(0, |dv| dv.path_or_inline_dv.len()),
    ])?;
    if let Some(tags) = &value.tags {
        bytes = add(
            bytes,
            map_layout::<String, Option<String>>(tags.capacity())?,
        )?;
        for (key, value) in tags {
            bytes = add(
                bytes,
                add(key.len(), value.as_ref().map_or(0, String::len))?,
            )?;
        }
    }
    Ok(bytes)
}

/// Complete owned CRC copy geometry; shared scope bookkeeping is separate and not copied.
pub(crate) fn clone_bytes(crc: &Crc) -> DeltaResult<usize> {
    let mut bytes = sum([
        size_of::<Crc>(),
        2 * size_of::<usize>(),
        metadata_bytes(&crc.metadata)?,
        protocol_bytes(&crc.protocol)?,
        domain_map(domain_state(&crc.domain_metadata_state))?,
        transaction_map(transaction_state(&crc.set_transaction_state))?,
        crc.txn_id.as_ref().map_or(0, String::len),
    ])?;
    if let Some(histogram) = crc.file_stats().and_then(|s| s.file_size_histogram()) {
        bytes = add(bytes, histogram_bytes(histogram)?)?;
    }
    if let Some(histogram) = &crc.deleted_record_counts_histogram_opt {
        bytes = add(
            bytes,
            mul(histogram.deleted_record_counts.len(), size_of::<i64>())?,
        )?;
    }
    if let Some(files) = &crc.all_files {
        bytes = add(bytes, mul(files.len(), size_of::<Add>())?)?;
        for file in files {
            bytes = add(bytes, add_bytes(file)?)?;
        }
    }
    Ok(bytes)
}

pub(crate) fn delta_bytes(delta: &CrcDelta) -> DeltaResult<usize> {
    let mut bytes = sum([
        size_of::<CrcDelta>(),
        domain_map(&delta.domain_metadata)?,
        transaction_map(&delta.set_transactions)?,
    ])?;
    if let Some(metadata) = &delta.metadata {
        bytes = add(bytes, metadata_bytes(metadata)?)?;
    }
    if let Some(protocol) = &delta.protocol {
        bytes = add(bytes, protocol_bytes(protocol)?)?;
    }
    if let Some(histogram) = &delta.file_stats.net_histogram {
        bytes = add(bytes, histogram_bytes(histogram)?)?;
    }
    Ok(bytes)
}

/// Original-JSON-source geometry, before any CrcRaw serde allocation.
/// The terms cover all Vec growth layouts, both map-construction phases, owned
/// and intermediate strings, parser scratch, and bounded native diagnostics.
pub(crate) fn decode_bytes(shape: JsonShape) -> DeltaResult<usize> {
    // Use the exact pinned enum layout, including any i128 alignment; the
    // private-version namespace intentionally makes a serde upgrade fail closed.
    type Content<'a> = serde::__private229::de::Content<'a>;
    let vector_slots = sum([
        size_of::<SetTransaction>(),
        size_of::<DomainMetadata>(),
        size_of::<TableFeature>(),
        size_of::<String>(),
        size_of::<i64>(),
        size_of::<Content<'_>>(),
        size_of::<(Content<'_>, Content<'_>)>(),
    ])?;
    let map_slots = sum([
        size_of::<(String, String)>(),
        size_of::<(String, DomainMetadata)>(),
        size_of::<(String, SetTransaction)>(),
    ])?;
    let elements = add(shape.tokens, mul(shape.containers, 8)?)?;
    // RawVec doubles: the sum of all new capacities is <4*(elements+minimums).
    let vectors = mul(mul(elements, 4)?, vector_slots)?;
    // Hashbrown load <=7/8 + power-of-two buckets: sum of growing layouts
    // is <=8*(entries+minimums), with a control byte per bucket and group padding.
    let maps = add(
        mul(mul(elements, 8)?, add(map_slots, 3)?)?,
        mul(add(shape.tokens, shape.containers)?, 3 * 32)?,
    )?;
    // Strings move into CrcRaw, except Serde Content/Unknown feature conversion
    // and the new app_id/domain map keys. Four owned-copy bytes per encoded byte
    // covers their entire allocation sum. Decoding escapes never expands UTF-8.
    // With arbitrary_precision enabled, malformed feature Content may own a
    // numeric lexeme as a String too. Whole source bytes bound both lexemes and
    // decoded strings even when the JSON string-byte count is zero.
    let strings = mul(shape.bytes, 4)?;
    let scratch = mul(add(shape.bytes, 8)?, 4)?;
    // Rust Debug string escapes use at most ten ASCII bytes per input byte.
    // Each tagged-feature attempt can format one fixed expected-variant list.
    // Pinned TableFeature has <=64-byte serialized names (checked source audit).
    let static_error = add(mul(<TableFeature as strum::EnumCount>::COUNT, 68)?, 256)?;
    let diagnostics = mul(
        add(
            mul(shape.bytes, 10)?,
            mul(add(shape.tokens, 1)?, static_error)?,
        )?,
        4,
    )?;
    sum([
        vectors,
        maps,
        strings,
        scratch,
        diagnostics,
        size_of::<CrcRaw>(),
        size_of::<Crc>(),
        size_of::<ScopedCrcError>(),
        4 * size_of::<usize>(),
    ])
}

#[derive(Debug)]
struct ScopedCrcError {
    error: Error,
    _scope: Arc<NativeResourceScope>,
}
impl std::fmt::Display for ScopedCrcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ScopedCrcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
pub(crate) fn retain_error(error: Error, scope: &Option<Arc<NativeResourceScope>>) -> Error {
    if error.is_resource_exhausted() {
        return error;
    }
    match scope {
        Some(scope) => Error::generic_err(ScopedCrcError {
            error,
            _scope: scope.clone(),
        }),
        None => error,
    }
}
pub(crate) fn reserve(
    scope: &Option<Arc<NativeResourceScope>>,
    kind: &'static str,
    bytes: usize,
) -> DeltaResult<()> {
    if let Some(scope) = scope {
        scope.reserve(AllocationRequest { kind, bytes })?;
    }
    Ok(())
}

pub(crate) fn select_scope(
    explicit: Option<Arc<NativeResourceScope>>,
    original: Option<Arc<NativeResourceScope>>,
) -> DeltaResult<Option<Arc<NativeResourceScope>>> {
    let current = crate::resource::current_resource_scope();
    if let (Some(explicit), Some(current)) = (&explicit, &current) {
        if !Arc::ptr_eq(explicit, current) {
            return Err(ResourceExhausted {
                kind: "native_foreign_scope",
                requested: 1,
                limit: 0,
            }
            .into());
        }
    }
    Ok(current.or(explicit).or(original))
}

pub(crate) fn apply_bytes(crc: &Crc, delta: &CrcDelta) -> DeltaResult<usize> {
    // Adopt all moved backing into the new scope before dropping any predecessor
    // scope. This intentionally overcharges a transfer until predecessor owners drop.
    let domains = map_layout::<String, DomainMetadata>(add(
        domain_state(&crc.domain_metadata_state).capacity(),
        delta.domain_metadata.len(),
    )?)?;
    let transactions = map_layout::<String, SetTransaction>(add(
        transaction_state(&crc.set_transaction_state).capacity(),
        delta.set_transactions.len(),
    )?)?;
    let histogram = crc
        .file_stats()
        .and_then(|s| s.file_size_histogram())
        .map(histogram_bytes)
        .transpose()?
        .unwrap_or(0);
    sum([
        clone_bytes(crc)?,
        delta_bytes(delta)?,
        domains,
        transactions,
        histogram,
        diagnostic_bytes(0)?,
    ])
}

pub(crate) fn diagnostic_bytes(string_bytes: usize) -> DeltaResult<usize> {
    // Every fixed CRC visitor/histogram message is shorter than 256 bytes;
    // at most three decimal usize/i64 fields add <=60 bytes. Four formatted
    // allocations cover format!, native error wrapping, serde custom, and Box<str>.
    mul(add(mul(string_bytes, 10)?, 512)?, 4)
}
