// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.
use std::io::Write;
use std::sync::Arc;

use super::encoder_resource as resource;
use crate::StructMode;
use arrow_array::cast::AsArray;
use arrow_array::types::*;
use arrow_array::*;
use arrow_buffer::{ArrowNativeType, NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow_cast::display::{ArrayFormatter, FormatOptions};
use arrow_schema::{ArrowError, DataType, FieldRef};
use half::f16;
use lexical_core::FormattedSize;
use serde_core::Serializer;

/// Configuration options for the JSON encoder.
#[derive(Debug, Clone, Default)]
pub struct EncoderOptions {
    /// Whether to include nulls in the output or elide them.
    explicit_nulls: bool,
    explicit_nulls_in_maps: bool,
    /// Whether to encode structs as JSON objects or JSON arrays of their values.
    struct_mode: StructMode,
    /// An optional hook for customizing encoding behavior.
    encoder_factory: Option<Arc<dyn EncoderFactory>>,
    /// Optional date format for date arrays
    date_format: Option<String>,
    /// Optional datetime format for datetime arrays
    datetime_format: Option<String>,
    /// Optional timestamp format for timestamp arrays
    timestamp_format: Option<String>,
    /// Optional timestamp format for timestamp with timezone arrays
    timestamp_tz_format: Option<String>,
    /// Optional time format for time arrays
    time_format: Option<String>,
}

impl EncoderOptions {
    /// Set whether to include nulls in the output or elide them.
    pub fn with_explicit_nulls(mut self, explicit_nulls: bool) -> Self {
        self.explicit_nulls = explicit_nulls;
        self
    }

    /// Preserve null map values while keeping ordinary null struct fields elided.
    pub fn with_explicit_nulls_in_maps(mut self, explicit: bool) -> Self {
        self.explicit_nulls_in_maps = explicit;
        self
    }

    /// Set whether to encode structs as JSON objects or JSON arrays of their values.
    pub fn with_struct_mode(mut self, struct_mode: StructMode) -> Self {
        self.struct_mode = struct_mode;
        self
    }

    /// Set an optional hook for customizing encoding behavior.
    pub fn with_encoder_factory(mut self, encoder_factory: Arc<dyn EncoderFactory>) -> Self {
        self.encoder_factory = Some(encoder_factory);
        self
    }

    /// Get whether to include nulls in the output or elide them.
    pub fn explicit_nulls(&self) -> bool {
        self.explicit_nulls
    }

    /// Get whether to encode structs as JSON objects or JSON arrays of their values.
    pub fn struct_mode(&self) -> StructMode {
        self.struct_mode
    }

    /// Get the optional hook for customizing encoding behavior.
    pub fn encoder_factory(&self) -> Option<&Arc<dyn EncoderFactory>> {
        self.encoder_factory.as_ref()
    }

    /// Set the JSON file's date format
    pub fn with_date_format(mut self, format: String) -> Self {
        self.date_format = Some(format);
        self
    }

    /// Get the JSON file's date format if set, defaults to RFC3339
    pub fn date_format(&self) -> Option<&str> {
        self.date_format.as_deref()
    }

    /// Set the JSON file's datetime format
    pub fn with_datetime_format(mut self, format: String) -> Self {
        self.datetime_format = Some(format);
        self
    }

    /// Get the JSON file's datetime format if set, defaults to RFC3339
    pub fn datetime_format(&self) -> Option<&str> {
        self.datetime_format.as_deref()
    }

    /// Set the JSON file's time format
    pub fn with_time_format(mut self, format: String) -> Self {
        self.time_format = Some(format);
        self
    }

    /// Get the JSON file's datetime time if set, defaults to RFC3339
    pub fn time_format(&self) -> Option<&str> {
        self.time_format.as_deref()
    }

    /// Set the JSON file's timestamp format
    pub fn with_timestamp_format(mut self, format: String) -> Self {
        self.timestamp_format = Some(format);
        self
    }

    /// Get the JSON file's timestamp format if set, defaults to RFC3339
    pub fn timestamp_format(&self) -> Option<&str> {
        self.timestamp_format.as_deref()
    }

    /// Set the JSON file's timestamp tz format
    pub fn with_timestamp_tz_format(mut self, tz_format: String) -> Self {
        self.timestamp_tz_format = Some(tz_format);
        self
    }

    /// Get the JSON file's timestamp tz format if set, defaults to RFC3339
    pub fn timestamp_tz_format(&self) -> Option<&str> {
        self.timestamp_tz_format.as_deref()
    }
}

/// A trait to create custom encoders for specific data types.
///
/// This allows overriding the default encoders for specific data types,
/// or adding new encoders for custom data types.
///
/// # Examples
///
/// ```
/// use std::io::Write;
/// use arrow_array::{ArrayAccessor, Array, BinaryArray, Float64Array, RecordBatch};
/// use arrow_array::cast::AsArray;
/// use arrow_schema::{DataType, Field, Schema, FieldRef};
/// use arrow_json::{writer::{WriterBuilder, JsonArray, NullableEncoder}, StructMode};
/// use arrow_json::{Encoder, EncoderFactory, EncoderOptions};
/// use arrow_schema::ArrowError;
/// use std::sync::Arc;
/// use serde_json::json;
/// use serde_json::Value;
///
/// struct IntArrayBinaryEncoder<B> {
///     array: B,
/// }
///
/// impl<'a, B> Encoder for IntArrayBinaryEncoder<B>
/// where
///     B: ArrayAccessor<Item = &'a [u8]>,
/// {
///     fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
///         out.push(b'[');
///         let child = self.array.value(idx);
///         for (idx, byte) in child.iter().enumerate() {
///             write!(out, "{byte}").unwrap();
///             if idx < child.len() - 1 {
///                 out.push(b',');
///             }
///         }
///         out.push(b']');
///     }
/// }
///
/// #[derive(Debug)]
/// struct IntArayBinaryEncoderFactory;
///
/// impl EncoderFactory for IntArayBinaryEncoderFactory {
///     fn make_default_encoder<'a>(
///         &self,
///         _field: &'a FieldRef,
///         array: &'a dyn Array,
///         _options: &'a EncoderOptions,
///     ) -> Result<Option<NullableEncoder<'a>>, ArrowError> {
///         match array.data_type() {
///             DataType::Binary => {
///                 let array = array.as_binary::<i32>();
///                 let encoder = IntArrayBinaryEncoder { array };
///                 let array_encoder = Box::new(encoder) as Box<dyn Encoder + 'a>;
///                 let nulls = array.nulls().cloned();
///                 Ok(Some(NullableEncoder::new(array_encoder, nulls)))
///             }
///             _ => Ok(None),
///         }
///     }
/// }
///
/// let binary_array = BinaryArray::from_iter([Some(b"a".as_slice()), None, Some(b"b".as_slice())]);
/// let float_array = Float64Array::from(vec![Some(1.0), Some(2.3), None]);
/// let fields = vec![
///     Field::new("bytes", DataType::Binary, true),
///     Field::new("float", DataType::Float64, true),
/// ];
/// let batch = RecordBatch::try_new(
///     Arc::new(Schema::new(fields)),
///     vec![
///         Arc::new(binary_array) as Arc<dyn Array>,
///         Arc::new(float_array) as Arc<dyn Array>,
///     ],
/// )
/// .unwrap();
///
/// let json_value: Value = {
///     let mut buf = Vec::new();
///     let mut writer = WriterBuilder::new()
///         .with_encoder_factory(Arc::new(IntArayBinaryEncoderFactory))
///         .build::<_, JsonArray>(&mut buf);
///     writer.write_batches(&[&batch]).unwrap();
///     writer.finish().unwrap();
///     serde_json::from_slice(&buf).unwrap()
/// };
///
/// let expected = json!([
///     {"bytes": [97], "float": 1.0},
///     {"float": 2.3},
///     {"bytes": [98]},
/// ]);
///
/// assert_eq!(json_value, expected);
/// ```
pub trait EncoderFactory: std::fmt::Debug + Send + Sync {
    /// Make an encoder that overrides the default encoder for a specific field and array or provides an encoder for a custom data type.
    /// This can be used to override how e.g. binary data is encoded so that it is an encoded string or an array of integers.
    ///
    /// Note that the type of the field may not match the type of the array: for dictionary arrays unless the top-level dictionary is handled this
    /// will be called again for the keys and values of the dictionary, at which point the field type will still be the outer dictionary type but the
    /// array will have a different type.
    /// For example, `field`` might have the type `Dictionary(i32, Utf8)` but `array` will be `Utf8`.
    fn make_default_encoder<'a>(
        &self,
        _field: &'a FieldRef,
        _array: &'a dyn Array,
        _options: &'a EncoderOptions,
    ) -> Result<Option<NullableEncoder<'a>>, ArrowError> {
        Ok(None)
    }
}

/// An encoder + a null buffer.
/// This is packaged together into a wrapper struct to minimize dynamic dispatch for null checks.
pub struct NullableEncoder<'a> {
    encoder: Box<dyn Encoder + 'a>,
    nulls: Option<NullBuffer>,
    // Owners follow encoder/null payload so receipts outlive all destructors.
    rows: Option<usize>,
    resource_policy: Option<crate::resource::ReaderResourcePolicy>,
    resource_owner: arrow_schema::resource::ResourceOwnerHandle,
}

impl<'a> NullableEncoder<'a> {
    /// Create a new encoder with a null buffer.
    pub fn new(encoder: Box<dyn Encoder + 'a>, nulls: Option<NullBuffer>) -> Self {
        Self {
            encoder,
            nulls,
            rows: None,
            resource_policy: crate::resource::ReaderResourcePolicy::current(),
            resource_owner: arrow_schema::resource::ResourceOwnerHandle::capture(),
        }
    }

    /// Return the exact encoded row length using the native encoder's borrowed
    /// values. Native formatter scratch is admitted before this counting pass.
    pub fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        if self.rows.is_some_and(|rows| idx >= rows) {
            return Err(resource::failure(
                "JSON encoder index",
                idx.saturating_add(1),
                self.rows.unwrap(),
            ));
        }
        crate::resource::validate_reader_owner(self.resource_policy.as_ref())?;
        match (
            self.resource_owner.owner(),
            arrow_schema::resource::current_resource_owner(),
        ) {
            (Some(original), Some(current)) => {
                if !Arc::ptr_eq(original, &current) {
                    current.try_adopt(original.clone())?;
                }
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(resource::failure("JSON prepared encoder Arrow owner", 1, 0));
            }
            (None, None) => {}
        }
        crate::resource::scoped(self.resource_policy.as_ref(), || {
            self.encoder.try_encoded_len(idx)
        })
    }

    /// Reserve the complete replacement output capacity before native encoding.
    /// Unlike `encode`, this surface can return admission/geometry failure.
    pub fn try_encode(&mut self, idx: usize, out: &mut Vec<u8>) -> Result<(), ArrowError> {
        self.try_encode_with_limit(idx, out, isize::MAX as usize)
    }

    /// Encode one value while also enforcing a caller's output offset geometry.
    pub fn try_encode_with_limit(
        &mut self,
        idx: usize,
        out: &mut Vec<u8>,
        limit: usize,
    ) -> Result<(), ArrowError> {
        self.try_encode_row(idx, out, limit, false)
    }

    /// Encode one line-delimited JSON object with its native newline terminator.
    pub fn try_encode_line(&mut self, idx: usize, out: &mut Vec<u8>) -> Result<(), ArrowError> {
        self.try_encode_row(idx, out, isize::MAX as usize, true)
    }

    fn try_encode_row(
        &mut self,
        idx: usize,
        out: &mut Vec<u8>,
        limit: usize,
        line: bool,
    ) -> Result<(), ArrowError> {
        let size = resource::add(self.try_encoded_len(idx)?, usize::from(line))?;
        let required = resource::add(out.len(), size)?;
        if required > limit {
            return Err(resource::failure(
                "JSON caller output geometry",
                required,
                limit,
            ));
        }
        crate::resource::scoped(self.resource_policy.as_ref(), || {
            resource::output(out, size)?;
            self.encoder.encode(idx, out);
            if line {
                out.push(b'\n');
            }
            Ok(())
        })
    }

    /// Encode the value at index `idx` to `out`.
    pub fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        self.encoder.encode(idx, out)
    }

    /// Returns whether the value at index `idx` is null.
    pub fn is_null(&self, idx: usize) -> bool {
        self.nulls.as_ref().is_some_and(|nulls| nulls.is_null(idx))
    }

    /// Returns whether the encoder has any nulls.
    pub fn has_nulls(&self) -> bool {
        match self.nulls {
            Some(ref nulls) => nulls.null_count() > 0,
            None => false,
        }
    }
}

impl Encoder for NullableEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        NullableEncoder::try_encoded_len(self, idx)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        self.encoder.encode(idx, out)
    }
}

/// A trait to format array values as JSON values
///
/// Nullability is handled by the caller to allow encoding nulls implicitly, i.e. `{}` instead of `{"a": null}`
pub trait Encoder {
    /// Exact native output size. Custom encoders must implement this fallible
    /// seam before use through `NullableEncoder::try_encode`.
    fn try_encoded_len(&mut self, _idx: usize) -> Result<usize, ArrowError> {
        Err(resource::failure(
            "JSON encoder has no admitted size path",
            1,
            0,
        ))
    }
    /// Encode the non-null value at index `idx` to `out`.
    ///
    /// The behaviour is unspecified if `idx` corresponds to a null index.
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>);
}

/// Creates an encoder for the given array and field.
///
/// This first calls the EncoderFactory if one is provided, and then falls back to the default encoders.
pub fn make_encoder<'a>(
    field: &'a FieldRef,
    array: &'a dyn Array,
    options: &'a EncoderOptions,
) -> Result<NullableEncoder<'a>, ArrowError> {
    let _depth = resource::Depth::enter()?;
    // The prepared path proves the installed native default formatter. User
    // formats can change scratch and escaping behavior and need a separate
    // admitted formatter implementation before use under a required owner.
    if resource::governed()
        && (options.date_format.is_some()
            || options.datetime_format.is_some()
            || options.timestamp_format.is_some()
            || options.timestamp_tz_format.is_some()
            || options.time_format.is_some())
    {
        return Err(resource::failure(
            "JSON custom temporal format admission",
            1,
            0,
        ));
    }
    if let Some(policy) = crate::resource::ReaderResourcePolicy::current() {
        if array.len() > policy.limits().collection_entries {
            return Err(resource::failure(
                "JSON encoder values",
                array.len(),
                policy.limits().collection_entries,
            ));
        }
    }
    macro_rules! primitive_helper {
        ($t:ty) => {{
            let array = array.as_primitive::<$t>();
            let nulls = array.nulls().cloned();
            NullableEncoder::new(resource::boxed(PrimitiveEncoder::new(array))?, nulls)
        }};
    }

    if let Some(factory) = options.encoder_factory() {
        if resource::governed() {
            return Err(resource::failure(
                "JSON custom encoder factory admission",
                1,
                0,
            ));
        }
        if let Some(encoder) = factory.make_default_encoder(field, array, options)? {
            return Ok(encoder);
        }
    }

    let nulls = array.nulls().cloned();
    let mut encoder = downcast_integer! {
        array.data_type() => (primitive_helper),
        DataType::Float16 => primitive_helper!(Float16Type),
        DataType::Float32 => primitive_helper!(Float32Type),
        DataType::Float64 => primitive_helper!(Float64Type),
        DataType::Boolean => {
            let array = array.as_boolean();
            NullableEncoder::new(resource::boxed(BooleanEncoder(array))?, array.nulls().cloned())
        }
        DataType::Null => {
            resource::nulls(array.len())?;
            NullableEncoder::new(resource::boxed(NullEncoder)?, array.logical_nulls())
        },
        DataType::Utf8 => {
            let array = array.as_string::<i32>();
            NullableEncoder::new(resource::boxed(StringEncoder(array))?, array.nulls().cloned())
        }
        DataType::LargeUtf8 => {
            let array = array.as_string::<i64>();
            NullableEncoder::new(resource::boxed(StringEncoder(array))?, array.nulls().cloned())
        }
        DataType::Utf8View => {
            let array = array.as_string_view();
            NullableEncoder::new(resource::boxed(StringViewEncoder(array))?, array.nulls().cloned())
        }
        DataType::BinaryView => {
            let array = array.as_binary_view();
            NullableEncoder::new(resource::boxed(BinaryViewEncoder(array))?, array.nulls().cloned())
        }
        DataType::List(_) => {
            let array = array.as_list::<i32>();
            NullableEncoder::new(resource::boxed(ListLikeEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }
        DataType::LargeList(_) => {
            let array = array.as_list::<i64>();
            NullableEncoder::new(resource::boxed(ListLikeEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }
        DataType::ListView(_) => {
            let array = array.as_list_view::<i32>();
            NullableEncoder::new(resource::boxed(ListLikeEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }
        DataType::LargeListView(_) => {
            let array = array.as_list_view::<i64>();
            NullableEncoder::new(resource::boxed(ListLikeEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }
        DataType::FixedSizeList(_, _) => {
            let array = array.as_fixed_size_list();
            NullableEncoder::new(resource::boxed(ListLikeEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }

        DataType::Dictionary(_, _) => downcast_dictionary_array! {
            array => {
                NullableEncoder::new(resource::boxed(DictionaryEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
            },
            _ => unreachable!()
        }

        DataType::RunEndEncoded(_, _) if resource::governed() => return Err(resource::failure("JSON run-end encoder admission",1,0)),
        DataType::RunEndEncoded(_, _) => downcast_run_array! {
            array => {
                NullableEncoder::new(
                    resource::boxed(RunEndEncodedEncoder::try_new(field, array, options)?)?,
                    array.logical_nulls(),
                )
            },
            _ => unreachable!()
        }

        DataType::Map(_, _) => {
            let array = array.as_map();
            NullableEncoder::new(resource::boxed(MapEncoder::try_new(field, array, options)?)?, array.nulls().cloned())
        }

        DataType::FixedSizeBinary(_) => {
            let array = array.as_fixed_size_binary();
            NullableEncoder::new(resource::boxed(BinaryEncoder::new(array))? as _, array.nulls().cloned())
        }

        DataType::Binary => {
            let array: &BinaryArray = array.as_binary();
            NullableEncoder::new(resource::boxed(BinaryEncoder::new(array))?, array.nulls().cloned())
        }

        DataType::LargeBinary => {
            let array: &LargeBinaryArray = array.as_binary();
            NullableEncoder::new(resource::boxed(BinaryEncoder::new(array))?, array.nulls().cloned())
        }

        DataType::Struct(fields) => {
            let array = array.as_struct();
            let mut encoders=resource::vec(fields.len(),"JSON struct encoders")?;
            for (field, array) in fields.iter().zip(array.columns()) {
                let encoder=make_encoder(field,array,options)?;
                let size=resource::add(resource::string_len(field.name())?,1)?;
                let mut field_name=Vec::new();
                resource::output(&mut field_name,size)?;
                encode_string(field.name(), &mut field_name);
                field_name.push(b':');
                encoders.push(FieldEncoder{field_name,encoder});
            }

            let encoder = StructArrayEncoder{
                encoders,
                explicit_nulls: options.explicit_nulls(),
                struct_mode: options.struct_mode(),
            };
            let nulls = array.nulls().cloned();
            NullableEncoder::new(resource::boxed(encoder)? as Box<dyn Encoder + 'a>, nulls)
        }
        DataType::Decimal32(_, _) | DataType::Decimal64(_, _) | DataType::Decimal128(_, _) | DataType::Decimal256(_, _) => {
            resource::formatter::<(u8,i8)>()?;
            let options = FormatOptions::new().with_display_error(true);
            let formatter = JsonArrayFormatter::new(ArrayFormatter::try_new(array, &options)?, array.data_type());
            NullableEncoder::new(resource::boxed(RawArrayFormatter(formatter))? as Box<dyn Encoder + 'a>, nulls)
        }
        d => match d.is_temporal() {
            true => {
                // Note: the implementation of Encoder for ArrayFormatter assumes it does not produce
                // characters that would need to be escaped within a JSON string, e.g. `'"'`.
                // If support for user-provided format specifications is added, this assumption
                // may need to be revisited
                match d {
                    DataType::Timestamp(_,zone)=> {
                        resource::formatter::<(Option<arrow_array::timezone::Tz>,Option<&str>)>()?;
                        if let Some(zone)=zone { resource::bytes(resource::mul(resource::add(zone.len(),128)?,4)?,"JSON timezone diagnostic")?; }
                    },
                    DataType::Duration(_)=>resource::formatter::<arrow_cast::display::DurationFormat>()?,
                    DataType::Interval(_)=>resource::formatter::<()>()?,
                    _=>resource::formatter::<Option<&str>>()?,
                }
                let fops = FormatOptions::new().with_display_error(true)
                .with_date_format(options.date_format.as_deref())
                .with_datetime_format(options.datetime_format.as_deref())
                .with_timestamp_format(options.timestamp_format.as_deref())
                .with_timestamp_tz_format(options.timestamp_tz_format.as_deref())
                .with_time_format(options.time_format.as_deref());

                let formatter = ArrayFormatter::try_new(array, &fops)?;
                let formatter = JsonArrayFormatter::new(formatter,array.data_type());
                NullableEncoder::new(resource::boxed(formatter)? as Box<dyn Encoder + 'a>, nulls)
            }
            false => return Err(crate::resource::json_error(format_args!(
                "Unsupported data type for JSON encoding: {d:?}",
            )))
        }
    };

    encoder.rows = Some(array.len());
    Ok(encoder)
}

fn encode_string(s: &str, out: &mut Vec<u8>) {
    let mut serializer = serde_json::Serializer::new(out);
    serializer.serialize_str(s).unwrap();
}

fn encode_binary(bytes: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out.push(b'"');
}

struct FieldEncoder<'a> {
    field_name: Vec<u8>,
    encoder: NullableEncoder<'a>,
}

impl FieldEncoder<'_> {
    fn is_null(&self, idx: usize) -> bool {
        self.encoder.is_null(idx)
    }
}

struct StructArrayEncoder<'a> {
    encoders: Vec<FieldEncoder<'a>>,
    explicit_nulls: bool,
    struct_mode: StructMode,
}

impl Encoder for StructArrayEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        let mut size = 2;
        let mut first = true;
        let drop_nulls = self.struct_mode == StructMode::ObjectOnly && !self.explicit_nulls;
        for field in &mut self.encoders {
            let null = field.is_null(idx);
            if null && drop_nulls {
                continue;
            }
            if !first {
                size = resource::add(size, 1)?;
            }
            first = false;
            if self.struct_mode == StructMode::ObjectOnly {
                size = resource::add(size, field.field_name.len())?;
            }
            size = resource::add(
                size,
                if null {
                    4
                } else {
                    field.encoder.try_encoded_len(idx)?
                },
            )?;
        }
        Ok(size)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        match self.struct_mode {
            StructMode::ObjectOnly => out.push(b'{'),
            StructMode::ListOnly => out.push(b'['),
        }
        let mut is_first = true;
        // Nulls can only be dropped in explicit mode
        let drop_nulls = (self.struct_mode == StructMode::ObjectOnly) && !self.explicit_nulls;

        for field_encoder in self.encoders.iter_mut() {
            let is_null = field_encoder.is_null(idx);
            if is_null && drop_nulls {
                continue;
            }

            if !is_first {
                out.push(b',');
            }
            is_first = false;

            if self.struct_mode == StructMode::ObjectOnly {
                out.extend_from_slice(&field_encoder.field_name);
            }

            if is_null {
                out.extend_from_slice(b"null");
            } else {
                field_encoder.encoder.encode(idx, out);
            }
        }
        match self.struct_mode {
            StructMode::ObjectOnly => out.push(b'}'),
            StructMode::ListOnly => out.push(b']'),
        }
    }
}

trait PrimitiveEncode: ArrowNativeType {
    type Buffer;

    // Workaround https://github.com/rust-lang/rust/issues/61415
    fn init_buffer() -> Self::Buffer;

    /// Encode the primitive value as bytes, returning a reference to that slice.
    ///
    /// `buf` is temporary space that may be used
    fn encode(self, buf: &mut Self::Buffer) -> &[u8];
}

macro_rules! integer_encode {
    ($($t:ty),*) => {
        $(
            impl PrimitiveEncode for $t {
                type Buffer = [u8; Self::FORMATTED_SIZE];

                fn init_buffer() -> Self::Buffer {
                    [0; Self::FORMATTED_SIZE]
                }

                fn encode(self, buf: &mut Self::Buffer) -> &[u8] {
                    lexical_core::write(self, buf)
                }
            }
        )*
    };
}
integer_encode!(i8, i16, i32, i64, u8, u16, u32, u64);

macro_rules! float_encode {
    ($($t:ty),*) => {
        $(
            impl PrimitiveEncode for $t {
                type Buffer = [u8; Self::FORMATTED_SIZE];

                fn init_buffer() -> Self::Buffer {
                    [0; Self::FORMATTED_SIZE]
                }

                fn encode(self, buf: &mut Self::Buffer) -> &[u8] {
                    if self.is_infinite() || self.is_nan() {
                        b"null"
                    } else {
                        lexical_core::write(self, buf)
                    }
                }
            }
        )*
    };
}
float_encode!(f32, f64);

impl PrimitiveEncode for f16 {
    type Buffer = <f32 as PrimitiveEncode>::Buffer;

    fn init_buffer() -> Self::Buffer {
        f32::init_buffer()
    }

    fn encode(self, buf: &mut Self::Buffer) -> &[u8] {
        self.to_f32().encode(buf)
    }
}

struct PrimitiveEncoder<N: PrimitiveEncode> {
    values: ScalarBuffer<N>,
    buffer: N::Buffer,
}

impl<N: PrimitiveEncode> PrimitiveEncoder<N> {
    fn new<P: ArrowPrimitiveType<Native = N>>(array: &PrimitiveArray<P>) -> Self {
        Self {
            values: array.values().clone(),
            buffer: N::init_buffer(),
        }
    }
}

impl<N: PrimitiveEncode> Encoder for PrimitiveEncoder<N> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        Ok(self.values[idx].encode(&mut self.buffer).len())
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        out.extend_from_slice(self.values[idx].encode(&mut self.buffer));
    }
}

struct BooleanEncoder<'a>(&'a BooleanArray);

impl Encoder for BooleanEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        Ok(if self.0.value(idx) { 4 } else { 5 })
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        match self.0.value(idx) {
            true => out.extend_from_slice(b"true"),
            false => out.extend_from_slice(b"false"),
        }
    }
}

struct StringEncoder<'a, O: OffsetSizeTrait>(&'a GenericStringArray<O>);

impl<O: OffsetSizeTrait> Encoder for StringEncoder<'_, O> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        resource::string_len(self.0.value(idx))
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        encode_string(self.0.value(idx), out);
    }
}

struct StringViewEncoder<'a>(&'a StringViewArray);

impl Encoder for StringViewEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        resource::string_len(self.0.value(idx))
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        encode_string(self.0.value(idx), out);
    }
}

struct BinaryViewEncoder<'a>(&'a BinaryViewArray);

impl Encoder for BinaryViewEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        resource::add(resource::mul(self.0.value(idx).len(), 2)?, 2)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        encode_binary(self.0.value(idx), out);
    }
}

struct ListLikeEncoder<'a, L: ListLikeArray> {
    list_array: &'a L,
    encoder: NullableEncoder<'a>,
}

impl<'a, L: ListLikeArray> ListLikeEncoder<'a, L> {
    fn try_new(
        field: &'a FieldRef,
        array: &'a L,
        options: &'a EncoderOptions,
    ) -> Result<Self, ArrowError> {
        let encoder = make_encoder(field, array.values().as_ref(), options)?;
        Ok(Self {
            list_array: array,
            encoder,
        })
    }
}

impl<L: ListLikeArray> Encoder for ListLikeEncoder<'_, L> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        let range = self.list_array.element_range(idx);
        let mut size = 2;
        for child in range.clone() {
            if child != range.start {
                size = resource::add(size, 1)?;
            }
            size = resource::add(
                size,
                if self.encoder.is_null(child) {
                    4
                } else {
                    self.encoder.try_encoded_len(child)?
                },
            )?;
        }
        Ok(size)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        let range = self.list_array.element_range(idx);
        let start = range.start;
        let end = range.end;
        out.push(b'[');
        if self.encoder.has_nulls() {
            for idx in start..end {
                if idx != start {
                    out.push(b',')
                }
                if self.encoder.is_null(idx) {
                    out.extend_from_slice(b"null");
                } else {
                    self.encoder.encode(idx, out);
                }
            }
        } else {
            for idx in start..end {
                if idx != start {
                    out.push(b',')
                }
                self.encoder.encode(idx, out);
            }
        }
        out.push(b']');
    }
}

struct DictionaryEncoder<'a, K: ArrowDictionaryKeyType> {
    keys: ScalarBuffer<K::Native>,
    encoder: NullableEncoder<'a>,
}

impl<'a, K: ArrowDictionaryKeyType> DictionaryEncoder<'a, K> {
    fn try_new(
        field: &'a FieldRef,
        array: &'a DictionaryArray<K>,
        options: &'a EncoderOptions,
    ) -> Result<Self, ArrowError> {
        let encoder = make_encoder(field, array.values().as_ref(), options)?;

        Ok(Self {
            keys: array.keys().values().clone(),
            encoder,
        })
    }
}

impl<K: ArrowDictionaryKeyType> Encoder for DictionaryEncoder<'_, K> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        self.encoder.try_encoded_len(self.keys[idx].as_usize())
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        self.encoder.encode(self.keys[idx].as_usize(), out)
    }
}

struct RunEndEncodedEncoder<'a, R: RunEndIndexType> {
    run_array: &'a RunArray<R>,
    encoder: NullableEncoder<'a>,
}

impl<'a, R: RunEndIndexType> RunEndEncodedEncoder<'a, R> {
    fn try_new(
        field: &'a FieldRef,
        array: &'a RunArray<R>,
        options: &'a EncoderOptions,
    ) -> Result<Self, ArrowError> {
        let encoder = make_encoder(field, array.values().as_ref(), options)?;
        Ok(Self {
            run_array: array,
            encoder,
        })
    }
}

impl<R: RunEndIndexType> Encoder for RunEndEncodedEncoder<'_, R> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        self.encoder
            .try_encoded_len(self.run_array.get_physical_index(idx))
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        let physical_idx = self.run_array.get_physical_index(idx);
        self.encoder.encode(physical_idx, out)
    }
}

/// A newtype wrapper around [`ArrayFormatter`] to keep our usage of it private and not implement `Encoder` for the public type
struct JsonArrayFormatter<'a> {
    formatter: ArrayFormatter<'a>,
    kind: &'a DataType,
}

impl<'a> JsonArrayFormatter<'a> {
    fn new(formatter: ArrayFormatter<'a>, kind: &'a DataType) -> Self {
        Self { formatter, kind }
    }
    fn encoded_len(&self, idx: usize) -> Result<usize, ArrowError> {
        resource::formatter_scratch(self.kind)?;
        struct Count(usize);
        impl std::fmt::Write for Count {
            fn write_str(&mut self, value: &str) -> std::fmt::Result {
                self.0 = self.0.checked_add(value.len()).ok_or(std::fmt::Error)?;
                Ok(())
            }
        }
        let mut count = Count(0);
        std::fmt::write(&mut count, format_args!("{}", self.formatter.value(idx))).map_err(
            |_| {
                resource::failure(
                    "JSON native formatter length",
                    usize::MAX,
                    isize::MAX as usize,
                )
            },
        )?;
        Ok(count.0)
    }
}

impl Encoder for JsonArrayFormatter<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        resource::add(self.encoded_len(idx)?, 2)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        out.push(b'"');
        // Should be infallible
        // Note: We are making an assumption that the formatter does not produce characters that require escaping
        let _ = write!(out, "{}", self.formatter.value(idx));
        out.push(b'"')
    }
}

/// A newtype wrapper around [`JsonArrayFormatter`] that skips surrounding the value with `"`
struct RawArrayFormatter<'a>(JsonArrayFormatter<'a>);

impl Encoder for RawArrayFormatter<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        self.0.encoded_len(idx)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        let _ = write!(out, "{}", self.0.formatter.value(idx));
    }
}

struct NullEncoder;

impl Encoder for NullEncoder {
    fn try_encoded_len(&mut self, _idx: usize) -> Result<usize, ArrowError> {
        Err(resource::failure(
            "JSON null value must be handled by nullable parent",
            1,
            0,
        ))
    }
    fn encode(&mut self, _idx: usize, _out: &mut Vec<u8>) {
        unreachable!()
    }
}

struct MapEncoder<'a> {
    offsets: OffsetBuffer<i32>,
    keys: NullableEncoder<'a>,
    values: NullableEncoder<'a>,
    explicit_nulls: bool,
}

impl<'a> MapEncoder<'a> {
    fn try_new(
        field: &'a FieldRef,
        array: &'a MapArray,
        options: &'a EncoderOptions,
    ) -> Result<Self, ArrowError> {
        let values = array.values();
        let keys = array.keys();

        if !matches!(
            keys.data_type(),
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
        ) {
            return Err(ArrowError::JsonError(format!(
                "Only UTF8 keys supported by JSON MapArray Writer: got {:?}",
                keys.data_type()
            )));
        }

        let keys = make_encoder(field, keys, options)?;
        let values = make_encoder(field, values, options)?;

        // We sanity check nulls as these are currently not enforced by MapArray (#1697)
        if keys.has_nulls() {
            return Err(ArrowError::InvalidArgumentError(
                "Encountered nulls in MapArray keys".to_string(),
            ));
        }

        if array.entries().nulls().is_some_and(|x| x.null_count() != 0) {
            return Err(ArrowError::InvalidArgumentError(
                "Encountered nulls in MapArray entries".to_string(),
            ));
        }

        Ok(Self {
            offsets: array.offsets().clone(),
            keys,
            values,
            explicit_nulls: options.explicit_nulls() || options.explicit_nulls_in_maps,
        })
    }
}

impl Encoder for MapEncoder<'_> {
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        let mut size = 2;
        let mut first = true;
        for child in self.offsets[idx].as_usize()..self.offsets[idx + 1].as_usize() {
            let null = self.values.is_null(child);
            if null && !self.explicit_nulls {
                continue;
            }
            if !first {
                size = resource::add(size, 1)?;
            }
            first = false;
            size = resource::add(size, self.keys.try_encoded_len(child)?)?;
            size = resource::add(size, 1)?;
            size = resource::add(
                size,
                if null {
                    4
                } else {
                    self.values.try_encoded_len(child)?
                },
            )?;
        }
        Ok(size)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        let end = self.offsets[idx + 1].as_usize();
        let start = self.offsets[idx].as_usize();

        let mut is_first = true;

        out.push(b'{');

        for idx in start..end {
            let is_null = self.values.is_null(idx);
            if is_null && !self.explicit_nulls {
                continue;
            }

            if !is_first {
                out.push(b',');
            }
            is_first = false;

            self.keys.encode(idx, out);
            out.push(b':');

            if is_null {
                out.extend_from_slice(b"null");
            } else {
                self.values.encode(idx, out);
            }
        }
        out.push(b'}');
    }
}

/// New-type wrapper for encoding the binary types in arrow: `Binary`, `LargeBinary`
/// and `FixedSizeBinary` as hex strings in JSON.
struct BinaryEncoder<B>(B);

impl<'a, B> BinaryEncoder<B>
where
    B: ArrayAccessor<Item = &'a [u8]>,
{
    fn new(array: B) -> Self {
        Self(array)
    }
}

impl<'a, B> Encoder for BinaryEncoder<B>
where
    B: ArrayAccessor<Item = &'a [u8]>,
{
    fn try_encoded_len(&mut self, idx: usize) -> Result<usize, ArrowError> {
        resource::add(resource::mul(self.0.value(idx).len(), 2)?, 2)
    }
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        out.push(b'"');
        for byte in self.0.value(idx) {
            // this write is infallible
            write!(out, "{byte:02x}").unwrap();
        }
        out.push(b'"');
    }
}
