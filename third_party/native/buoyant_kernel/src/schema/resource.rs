//! Original schema decode admission and retained owner.
//!
//! This bound is specific to the native schema serde path, not arbitrary Rust
//! deserialization. See RESOURCE_BOUND.md for the source and phase inventory.

use std::mem::{align_of, size_of};
use std::sync::Arc;

use crate::resource::{
    AllocationRequest, JsonResourceLimits, JsonShape, NativeResourceScope, ResourceExhausted,
};
use crate::{DeltaResult, Error};

use super::{
    ArrayType, DataType, MapType, MetadataColumnSpec, MetadataValue, StructField, StructType,
};

/// An infallible compatibility Clone is an unadmitted deep copy. A scope is only
/// installed after explicit admission of a decode/copy, never by cloning this field.
#[derive(Debug, Default)]
pub(super) struct SchemaResourceOwner {
    scope: Option<Arc<NativeResourceScope>>,
    retained_bytes: usize,
    source_shape: Option<JsonShape>,
}

impl Clone for SchemaResourceOwner {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PartialEq for SchemaResourceOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for SchemaResourceOwner {}

fn overflow() -> Error {
    ResourceExhausted {
        kind: "schema_allocation_layout",
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}
fn add(a: usize, b: usize) -> DeltaResult<usize> {
    a.checked_add(b)
        .filter(|value| *value <= isize::MAX as usize)
        .ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> DeltaResult<usize> {
    a.checked_mul(b)
        .filter(|value| *value <= isize::MAX as usize)
        .ok_or_else(overflow)
}
fn sum(values: impl IntoIterator<Item = usize>) -> DeltaResult<usize> {
    values.into_iter().try_fold(0, add)
}

/// An upper layout for a Rust aggregate without depending on private field order.
/// Allow padding before every field and tail padding, each below maximum alignment.
fn aggregate(parts: &[(usize, usize)]) -> DeltaResult<usize> {
    let alignment = parts.iter().map(|(_, a)| *a).max().unwrap_or(1);
    add(
        sum(parts.iter().map(|(size, _)| *size))?,
        mul(add(parts.len(), 1)?, alignment - 1)?,
    )
}
fn part<T>() -> (usize, usize) {
    (size_of::<T>(), align_of::<T>())
}

/// Maximum internal B-tree node layout in pinned Rust 1.98.0: B=6, eleven
/// key/value slots, parent pointer and two u16 fields, plus twelve child pointers.
/// A leaf is smaller. This is a layout upper bound, not a guess from final RSS.
fn json_btree_node() -> DeltaResult<usize> {
    aggregate(&[
        part::<usize>(),
        part::<u16>(),
        part::<u16>(),
        part::<[String; 11]>(),
        part::<[serde_json::Value; 11]>(),
        part::<[usize; 12]>(),
    ])
}

/// Sum of requested layouts in a growing hash table for at most `elements`
/// insertions including empty-container minimums. Pinned hashbrown uses 7/8
/// load, power-of-two bucket counts, one control byte/bucket, and <=16 tail bytes.
fn growing_hash_table<K, V>(elements: usize) -> DeltaResult<usize> {
    sum([
        mul(mul(elements, 8)?, add(size_of::<(K, V)>(), 1)?)?,
        mul(elements, add(align_of::<(K, V)>(), 32)?)?,
    ])
}

fn index_bucket<K, V>() -> DeltaResult<usize> {
    // indexmap::Bucket { hash: HashValue(usize), key: K, value: V }.
    aggregate(&[part::<usize>(), part::<K>(), part::<V>()])
}

/// Exact selected ToSchema constructor families. Generated field payload bytes
/// include both field/key name copies and recursively constructed field types.
/// new_unchecked inserts into an initially empty IndexMap; derive metadata is
/// empty, so its metadata-column HashMap allocates nothing.
pub(crate) fn derived_struct_bytes(fields: usize, field_payload_bytes: usize) -> DeltaResult<usize> {
    let entries = if fields == 0 { 0 } else { mul(mul(fields.max(4), 4)?, index_bucket::<String, StructField>()?)? };
    sum([6, field_payload_bytes, entries, growing_hash_table::<usize, ()>(fields)?])
}

/// Attach only after the fallible derived constructor has admitted its complete
/// original allocation bound. Do not fabricate a JSON source shape for a schema
/// constructed directly from the native typed action definition.
pub(crate) fn attach_derived_schema_owner(mut schema: StructType, scope: Arc<NativeResourceScope>, retained_bytes: usize) -> DeltaResult<StructType> {
    scope.check_available()?;
    schema.resource_owner = SchemaResourceOwner { scope: Some(scope), retained_bytes, source_shape: None };
    Ok(schema)
}

/// Borrowed deep-copy layout for derived visitor types. Original admitted schema
/// containers reuse their recorded source bound. Fresh derived schemas have no
/// metadata; arbitrary metadata-bearing compatibility types need their original
/// container's admission instead of pretending a name-only bound covers them.
pub(crate) fn type_copy_bytes(data_type: &DataType) -> DeltaResult<usize> {
    fn schema_bytes(schema: &StructType, depth: usize) -> DeltaResult<usize> {
        if schema.resource_owner.scope.is_some() { return Ok(schema.resource_owner.retained_bytes); }
        if !schema.metadata_columns.is_empty() || schema.fields().any(|field| !field.metadata.is_empty()) {
            return Err(ResourceExhausted { kind: "native derived type metadata copy", requested: 1, limit: 0 }.into());
        }
        let mut payload = schema.type_name.len();
        // IndexMap clone copies original hash-table capacity, while its entry
        // Vec clone uses the original entry length. The conservative cumulative
        // insertion bound at actual capacity dominates both original layouts.
        payload = add(payload, mul(mul(schema.fields.capacity().max(4), 4)?, index_bucket::<String, StructField>()?)?)?;
        payload = add(payload, growing_hash_table::<usize, ()>(schema.fields.capacity())?)?;
        for (key, field) in &schema.fields {
            payload = sum([payload, key.len(), field.name.len(), copy(&field.data_type, depth + 1)?])?;
        }
        Ok(payload)
    }
    fn copy(data_type: &DataType, depth: usize) -> DeltaResult<usize> {
        let limit = crate::resource::current_resource_scope().map_or(64, |scope| crate::resource::current_json_resource_limits().map_or(64, |limits| limits.max_depth));
        if depth > limit { return Err(ResourceExhausted { kind: "native derived type copy depth", requested: depth, limit }.into()); }
        match data_type {
            DataType::Primitive(_) => Ok(0),
            DataType::Array(array) => sum([size_of::<ArrayType>(), array.type_name.len(), copy(array.element_type(), depth + 1)?]),
            DataType::Map(map) => sum([size_of::<MapType>(), map.type_name.len(), copy(map.key_type(), depth + 1)?, copy(map.value_type(), depth + 1)?]),
            DataType::Struct(schema) | DataType::Variant(schema) => add(size_of::<StructType>(), schema_bytes(schema, depth)?),
        }
    }
    copy(data_type, 1)
}

/// Pre-admitted sum for native JSON -> Value/Content -> schema conversion.
/// All arithmetic is checked before serde runs. Input is scanned separately even
/// when the enclosing metadata string has already passed a log/CRC input check.
pub(super) fn decode_bytes(shape: JsonShape) -> DeltaResult<usize> {
    // Each DataType ancestor may visit a subtree again through Value. Four
    // passes per ancestor also cover MetadataValue's Content and Other(Value)
    // conversions and the typed final output. The JSON depth bounds ancestors.
    let passes = mul(add(shape.max_depth, 1)?, 4)?;
    // Each token could be the "variant" shorthand, which constructs two native
    // fields and a struct absent from JSON. Include those synthetic containers.
    let synthetic = mul(shape.tokens, 2)?;
    let tokens = add(shape.tokens, synthetic)?;
    let containers = add(shape.containers, shape.tokens)?;
    let elements = add(tokens, mul(containers, 8)?)?;
    let visits = mul(elements, passes)?;

    // Concrete native vector families; no largest-DTO multiplier. Content's
    // version-specific type is deliberately compiled against the exact serde pin.
    type Content = serde::__private229::de::Content<'static>;
    let vector_slots = sum([
        size_of::<serde_json::Value>(),
        size_of::<StructField>(),
        size_of::<Content>(),
        size_of::<(Content, Content)>(),
        index_bucket::<String, StructField>()?,
        index_bucket::<String, serde_json::Value>()?,
    ])?;
    let vectors = mul(mul(visits, 4)?, vector_slots)?;
    let maps = sum([
        growing_hash_table::<String, MetadataValue>(visits)?,
        growing_hash_table::<MetadataColumnSpec, usize>(visits)?,
        growing_hash_table::<String, ()>(visits)?,
        // IndexMap's separate index table, for fields and preserve_order Value.
        mul(growing_hash_table::<usize, ()>(visits)?, 2)?,
        // Include both Value map backends: feature unification may select either.
        mul(mul(visits, 2)?, json_btree_node()?)?,
    ])?;
    let boxes = mul(
        visits,
        sum([
            size_of::<StructType>(),
            size_of::<ArrayType>(),
            size_of::<MapType>(),
            size_of::<DataType>(),
            size_of::<Content>(),
        ])?,
    )?;

    // bytes includes numeric lexemes under arbitrary_precision and malformed
    // input, not only quoted strings. Variant adds "metadata", "value", "struct".
    let source_bytes = add(shape.bytes, mul(shape.tokens, 19)?)?;
    let string_bytes = mul(source_bytes, passes)?;
    let minimums = mul(mul(tokens, passes)?, 8)?;
    // parser scratch, owned Value/Content/typed strings, map-key copies, and
    // primitive names each use at most one source span per pass. RawVec's sum
    // of growth requests is <4*(final length+minimum initial capacity).
    let strings = mul(mul(add(string_bytes, minimums)?, 4)?, 5)?;
    // str::to_lowercase produces <=3 chars/scalar; each char uses <=4 UTF-8
    // bytes and each input scalar has >=1 byte. Include moving growth overlap.
    let lowercase = mul(add(mul(string_bytes, 12)?, minimums)?, 4)?;
    // Error formatting may escape every byte and repeatedly wrap an ancestor's
    // diagnostic. Ten bytes covers Debug escapes; native static schema/error
    // field lists fit in the separately audited 1024-byte term per token/pass.
    let diagnostics = mul(
        add(
            mul(string_bytes, 10)?,
            mul(mul(add(tokens, 1)?, passes)?, 1024)?,
        )?,
        4,
    )?;
    sum([
        vectors,
        maps,
        boxes,
        strings,
        lowercase,
        diagnostics,
        size_of::<StructType>(),
        size_of::<ScopedSchemaError>(),
        4 * size_of::<usize>(),
    ])
}

/// Final native backing bound, admitted independently before the decode starts.
/// Unlike decode_bytes, this has one final schema/metadata graph and no repeated
/// Value/Content, parser scratch, diagnostics, or duplicate-name validation set.
fn retained_bytes(shape: JsonShape) -> DeltaResult<usize> {
    let tokens = mul(shape.tokens, 3)?; // original plus two variant fields
    let containers = add(shape.containers, shape.tokens)?;
    let elements = add(tokens, mul(containers, 8)?)?;
    let vectors = mul(
        mul(elements, 2)?,
        sum([
            index_bucket::<String, StructField>()?,
            index_bucket::<String, serde_json::Value>()?,
            size_of::<serde_json::Value>(),
        ])?,
    )?;
    let maps = sum([
        growing_hash_table::<String, MetadataValue>(elements)?,
        growing_hash_table::<MetadataColumnSpec, usize>(elements)?,
        mul(growing_hash_table::<usize, ()>(elements)?, 2)?,
        mul(mul(elements, 2)?, json_btree_node()?)?,
    ])?;
    let strings = mul(
        add(add(shape.bytes, mul(shape.tokens, 19)?)?, mul(tokens, 8)?)?,
        8, // final geometric capacity <=2 times bytes, up to four key/value copies
    )?;
    sum([
        vectors,
        maps,
        strings,
        mul(
            tokens,
            sum([
                size_of::<StructType>(),
                size_of::<ArrayType>(),
                size_of::<MapType>(),
            ])?,
        )?,
        size_of::<StructType>(),
        2 * size_of::<usize>(),
    ])
}

#[derive(Debug)]
struct ScopedSchemaError {
    error: serde_json::Error,
    _scope: Arc<NativeResourceScope>,
    _scratch: Arc<dyn crate::resource::AllocationReceipt>,
}

#[derive(Debug)]
struct ScopedTransformError {
    error: Error,
    _scope: Arc<NativeResourceScope>,
    _scratch: Arc<dyn crate::resource::AllocationReceipt>,
}
impl std::fmt::Display for ScopedTransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ScopedTransformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Admission for the fixed native MakePhysical/filter transformations. This is
/// not a bound for arbitrary caller-supplied SchemaTransform implementations.
pub(super) struct SchemaTransformAdmission {
    scope: Arc<NativeResourceScope>,
    scratch: Arc<dyn crate::resource::AllocationReceipt>,
    retained_bytes: usize,
    shape: JsonShape,
}

/// Admission only for the installed StringifyFailureProneLeaves transform:
/// it replaces primitive leaves with the inline String variant, preserves all
/// names/metadata, and does not expand arrays, maps, variants, or fields. The
/// fixed physical-transform bound therefore dominates its output and scratch.
/// This is not an allocation contract for arbitrary SchemaTransform callers.
pub(crate) struct SchemaRelaxationAdmission(SchemaTransformAdmission);

impl SchemaRelaxationAdmission {
    pub(crate) fn begin(schema: &StructType) -> DeltaResult<Option<Self>> {
        Ok(SchemaTransformAdmission::begin(schema)?.map(Self))
    }

    pub(crate) fn finish(self, schema: StructType) -> DeltaResult<StructType> {
        self.0.finish(Ok(schema))
    }
}

impl SchemaTransformAdmission {
    pub(super) fn begin(schema: &StructType) -> DeltaResult<Option<Self>> {
        let Some(scope) = crate::resource::current_resource_scope() else {
            return Ok(None);
        };
        scope.check_available()?;
        let shape = schema
            .resource_owner
            .source_shape
            .ok_or(ResourceExhausted {
                kind: "native_schema_unadmitted_transform_source",
                requested: 1,
                limit: 0,
            })?;
        // Every source token may synthesize two variant fields. Physical
        // mapping adds at most one fixed parquet.field.id metadata key per field.
        let fields = mul(shape.tokens, 3)?;
        let elements = add(fields, mul(add(shape.containers, shape.tokens)?, 8)?)?;
        let added_metadata = sum([
            growing_hash_table::<String, MetadataValue>(elements)?,
            mul(fields, "parquet.field.id".len())?,
            // A physical name is copied from an existing metadata string while
            // that string remains in the output metadata map.
            mul(add(shape.bytes, mul(fields, 8)?)?, 2)?,
        ])?;
        let retained_bytes = add(schema.resource_owner.retained_bytes, added_metadata)?;
        scope.reserve(AllocationRequest {
            kind: "native_schema_transform_retained",
            bytes: retained_bytes,
        })?;
        // A changed child moves through its parents. A borrowed child can be
        // copied at each ancestor; allow the full original backing per ancestor
        // plus both old/new layouts, independently of the new final reservation.
        let copies = mul(
            schema.resource_owner.retained_bytes,
            mul(add(shape.max_depth, 1)?, 2)?,
        )?;
        let visitors = sum([
            mul(
                mul(elements, 4)?,
                sum([
                    size_of::<std::borrow::Cow<'static, StructField>>(),
                    size_of::<StructField>(),
                    size_of::<&str>(),
                    size_of::<std::collections::HashMap<&str, &str>>(),
                ])?,
            )?,
            growing_hash_table::<i64, &str>(elements)?,
            growing_hash_table::<&str, &str>(elements)?,
        ])?;
        let scratch = scope.reserve_transient(AllocationRequest {
            kind: "native_schema_transform",
            bytes: sum([
                copies,
                visitors,
                added_metadata,
                decode_bytes(shape)?,
                size_of::<ScopedTransformError>(),
            ])?,
        })?;
        Ok(Some(Self {
            scope,
            scratch,
            retained_bytes,
            shape,
        }))
    }

    pub(super) fn finish(self, result: DeltaResult<StructType>) -> DeltaResult<StructType> {
        self.scope.check_available()?;
        match result {
            Ok(mut schema) => {
                schema.resource_owner = SchemaResourceOwner {
                    scope: Some(self.scope),
                    retained_bytes: self.retained_bytes,
                    source_shape: Some(self.shape),
                };
                Ok(schema)
            }
            Err(error) if error.is_resource_exhausted() => Err(error),
            Err(error) => Err(Error::generic_err(ScopedTransformError {
                error,
                _scope: self.scope,
                _scratch: self.scratch,
            })),
        }
    }
}
impl std::fmt::Display for ScopedSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ScopedSchemaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl StructType {
    /// Bind an original schema decoded inside a separately pre-admitted native
    /// container. The caller's bound must include decode_bytes of this shape.
    pub(crate) fn attach_container_decode_owner(
        &mut self,
        scope: Arc<NativeResourceScope>,
        shape: JsonShape,
    ) -> DeltaResult<()> {
        self.resource_owner = SchemaResourceOwner {
            scope: Some(scope),
            retained_bytes: retained_bytes(shape)?,
            source_shape: Some(shape),
        };
        Ok(())
    }

    pub(crate) fn container_decode_bound(shape: JsonShape) -> DeltaResult<usize> {
        add(decode_bytes(shape)?, retained_bytes(shape)?)
    }

    /// Write the original native schema into its new metadata string. The
    /// caller must already own `scope` in that Metadata value. A counting sink
    /// determines output capacity without constructing a serialized copy.
    pub(crate) fn to_json_admitted(&self, scope: &Arc<NativeResourceScope>) -> DeltaResult<String> {
        use std::io::Write;
        crate::resource::ensure_required_scope(scope)?;
        scope.check_available()?;
        let scratch = scope.reserve_transient(AllocationRequest {
            kind: "native_schema_serialize",
            bytes: self.native_validation_bound()?,
        })?;
        struct Counter<'a> {
            bytes: usize,
            scope: &'a NativeResourceScope,
        }
        impl Write for Counter<'_> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let next = self
                    .bytes
                    .checked_add(bytes.len())
                    .filter(|size| *size <= isize::MAX as usize);
                let Some(next) = next else {
                    self.scope.record_failure(ResourceExhausted {
                        kind: "native_schema_serialized_bytes",
                        requested: usize::MAX,
                        limit: isize::MAX as usize,
                    });
                    return Err(std::io::ErrorKind::OutOfMemory.into());
                };
                self.bytes = next;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        struct Output<'a> {
            bytes: Vec<u8>,
            limit: usize,
            scope: &'a NativeResourceScope,
        }
        impl Write for Output<'_> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let next = self.bytes.len().checked_add(bytes.len());
                if next.is_none_or(|next| next > self.limit) {
                    self.scope.record_failure(ResourceExhausted {
                        kind: "native_schema_serialized_bytes",
                        requested: next.unwrap_or(usize::MAX),
                        limit: self.limit,
                    });
                    return Err(std::io::ErrorKind::OutOfMemory.into());
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let encode = || -> DeltaResult<String> {
            let mut counter = Counter { bytes: 0, scope };
            serde_json::to_writer(&mut counter, self).map_err(Error::MalformedJson)?;
            scope.check_available()?;
            scope.reserve(AllocationRequest {
                kind: "native_schema_serialized_output",
                bytes: counter.bytes,
            })?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(counter.bytes)
                .map_err(|_| ResourceExhausted {
                    kind: "native_schema_serialized_allocator",
                    requested: counter.bytes,
                    limit: 0,
                })?;
            let mut output = Output {
                bytes,
                limit: counter.bytes,
                scope,
            };
            serde_json::to_writer(&mut output, self).map_err(Error::MalformedJson)?;
            scope.check_available()?;
            String::from_utf8(output.bytes).map_err(|_| {
                Error::internal_error("native schema serializer produced invalid UTF-8")
            })
        };
        let result = encode();
        scope.check_available()?;
        result.map_err(|error| {
            if error.is_resource_exhausted() {
                return error;
            }
            Error::generic_err(ScopedTransformError {
                error,
                _scope: scope.clone(),
                _scratch: scratch,
            })
        })
    }

    /// Source inventory for TableConfiguration's fixed validation set: the
    /// timestamp/variant visitors use ZST outputs; the two Iceberg V3 visitors
    /// use Vec<String> paths and at most one rendered offender each. This does
    /// not admit arbitrary feature/plugin validators.
    pub(crate) fn native_validation_bound(&self) -> DeltaResult<usize> {
        let shape = self.resource_owner.source_shape.ok_or(ResourceExhausted {
            kind: "native_schema_unadmitted_validation_source",
            requested: 1,
            limit: 0,
        })?;
        let fields = mul(shape.tokens, 3)?;
        let paths = mul(add(fields, mul(add(shape.max_depth, 1)?, 8)?)?, 4)?;
        let vectors = mul(mul(paths, 2)?, size_of::<String>())?;
        let path_bytes = add(mul(shape.bytes, add(shape.max_depth, 1)?)?, mul(fields, 8)?)?;
        // Path copies and joining, plus Debug/Display of a complete offending
        // DataType. Native field metadata display may escape each source byte.
        let strings = mul(add(mul(path_bytes, 10)?, mul(add(fields, 1)?, 1024)?)?, 8)?;
        sum([vectors, strings, decode_bytes(shape)?])
    }

    /// Decode the native schema with admission before any serde allocation.
    /// The original schema owns its receipt scope; ordinary Arc clones share it.
    /// Compatibility deep Clone is intentionally unadmitted and cannot be used
    /// as a governed copy boundary.
    pub fn try_from_json_with_resources(
        encoded: &str,
        scope: Arc<NativeResourceScope>,
        limits: JsonResourceLimits,
    ) -> DeltaResult<Self> {
        // Select the dedicated worker's required policy before inspection or
        // admission; an explicitly looser schema policy cannot bypass it.
        let _guard = crate::resource::enter_resource_scope(Some(scope.clone()), Some(limits))?;
        let limits = crate::resource::current_json_resource_limits().unwrap_or(limits);
        let shape = limits.inspect(encoded.as_bytes())?;
        let retained_bytes = retained_bytes(shape)?;
        scope.reserve(AllocationRequest {
            kind: "native_schema_retained",
            bytes: retained_bytes,
        })?;
        let scratch = scope.reserve_transient(AllocationRequest {
            kind: "native_schema_decode",
            bytes: decode_bytes(shape)?,
        })?;
        crate::resource::with_resource_scope(scope.clone(), limits, || {
            let mut schema: Self = serde_json::from_str(encoded).map_err(|error| {
                Error::generic_err(ScopedSchemaError {
                    error,
                    _scope: scope.clone(),
                    _scratch: scratch.clone(),
                })
            })?;
            schema.resource_owner = SchemaResourceOwner {
                scope: Some(scope),
                retained_bytes,
                source_shape: Some(shape),
            };
            Ok(schema)
        })
    }

    /// Deep-copy a previously admitted original schema into a fresh admitted
    /// owner. Its source-derived final backing bound is retained with the
    /// original; admission precedes every allocation made by native Clone.
    /// Unadmitted compatibility values require a separately proved ingress.
    pub fn try_clone_with_resources(&self, scope: Arc<NativeResourceScope>) -> DeltaResult<Self> {
        crate::resource::ensure_required_scope(&scope)?;
        if self.resource_owner.scope.is_none() {
            return Err(ResourceExhausted {
                kind: "native_schema_unadmitted_copy_source",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        scope.reserve(AllocationRequest {
            kind: "native_schema_copy",
            bytes: self.resource_owner.retained_bytes,
        })?;
        let mut copied = self.clone();
        copied.resource_owner = SchemaResourceOwner {
            scope: Some(scope),
            retained_bytes: self.resource_owner.retained_bytes,
            source_shape: self.resource_owner.source_shape,
        };
        Ok(copied)
    }

    /// Borrow the actual original schema's admission owner without copying it.
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.resource_owner.scope.as_ref()
    }
}
