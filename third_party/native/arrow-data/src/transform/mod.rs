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

//! Low-level array data abstractions.
//!
//! Provides utilities for creating, manipulating, and converting Arrow arrays
//! made of primitive types, strings, and nested types.

use super::{ArrayData, ArrayDataBuilder, ByteView};
use crate::bit_mask::set_bits;
use arrow_buffer::buffer::{BooleanBuffer, NullBuffer};
use arrow_buffer::{ArrowNativeType, Buffer, IntervalMonthDayNano, MutableBuffer, bit_util, i256};
use arrow_schema::{ArrowError, DataType, IntervalUnit, UnionMode};
use half::f16;
use num_integer::Integer;
use std::mem;

mod boolean;
mod fixed_binary;
mod fixed_size_list;
mod list;
mod list_view;
mod null;
mod primitive;
mod resource;

/// Attach the current native owner to an immutable allocation that was admitted
/// before construction. This does not admit the payload or inspect its size.
/// Existing original buffer claims are preserved. The caller must discard the
/// fresh output on error; this helper does not make arbitrary mutations safe.
/// Empty buffers are included, since their allocation-owner metadata is real.
pub fn try_retain_fresh_buffer(buffer: &Buffer) -> Result<(), ArrowError> {
    let owner = arrow_schema::resource::ResourceOwnerHandle::capture();
    if owner.is_empty() {
        return Ok(());
    }
    #[cfg(feature = "pool")]
    return resource::retain_buffer_with_owner(buffer, owner);
    #[cfg(not(feature = "pool"))]
    {
        let _ = buffer;
        Err(arrow_schema::resource::ResourceOwnerError {
            kind: "fresh backing lifetime requires pool",
            requested: 1,
            limit: 0,
        }
        .into())
    }
}

mod run;
mod structure;
mod union;
mod utils;
mod variable_size;

type ExtendNullBits<'a> =
    Box<dyn Fn(&mut _MutableArrayData, usize, usize) -> Result<(), ArrowError> + 'a>;
// function that extends `[start..start+len]` to the mutable array.
// this is dynamic because different data_types influence how buffers and children are extended.
type Extend<'a> =
    Box<dyn Fn(&mut _MutableArrayData, usize, usize, usize) -> Result<(), ArrowError> + 'a>;

type ExtendNulls = Box<dyn Fn(&mut _MutableArrayData, usize) -> Result<(), ArrowError>>;

/// A mutable [ArrayData] that knows how to freeze itself into an [ArrayData].
/// This is just a data container.
#[derive(Debug)]
struct _MutableArrayData<'a> {
    pub data_type: DataType,
    pub null_count: usize,

    pub len: usize,
    pub null_buffer: Option<MutableBuffer>,

    // arrow specification only allows up to 3 buffers (2 ignoring the nulls above).
    // Thus, we place them in the stack to avoid bound checks and greater data locality.
    pub buffer1: MutableBuffer,
    pub buffer2: MutableBuffer,
    pub child_data: Vec<MutableArrayData<'a>>,
}

impl _MutableArrayData<'_> {
    fn null_buffer(&mut self) -> &mut MutableBuffer {
        self.null_buffer
            .as_mut()
            .expect("MutableArrayData not nullable")
    }
}

fn build_extend_null_bits(
    array: &ArrayData,
    use_nulls: bool,
) -> Result<ExtendNullBits<'_>, ArrowError> {
    if let Some(nulls) = array.nulls() {
        let bytes = nulls.validity();
        Ok(resource::boxed(
            move |mutable: &mut _MutableArrayData, start, len| {
                let mutable_len = mutable.len;
                let total = mutable_len
                    .checked_add(len)
                    .ok_or_else(|| resource::overflow("transform validity length"))?;
                let out = mutable.null_buffer();
                utils::resize_for_bits(out, total)?;
                mutable.null_count += set_bits(
                    out.as_slice_mut(),
                    bytes,
                    mutable_len,
                    nulls.offset() + start,
                    len,
                );
                Ok(())
            },
            "transform validity callback",
        )?)
    } else if use_nulls {
        Ok(resource::boxed(
            |mutable: &mut _MutableArrayData, _, len| {
                let mutable_len = mutable.len;
                let total = mutable_len
                    .checked_add(len)
                    .ok_or_else(|| resource::overflow("transform validity length"))?;
                let out = mutable.null_buffer();
                utils::resize_for_bits(out, total)?;
                let write_data = out.as_slice_mut();
                (0..len).for_each(|i| bit_util::set_bit(write_data, mutable_len + i));
                Ok(())
            },
            "transform validity callback",
        )?)
    } else {
        Ok(resource::boxed(
            |_: &mut _MutableArrayData, _, _| Ok(()),
            "transform validity callback",
        )?)
    }
}

/// Efficiently create an [ArrayData] from one or more existing [ArrayData]s by
/// copying chunks.
///
/// The main use case of this struct is to perform unary operations to arrays of
/// arbitrary types, such as `filter` and `take`.
///
/// # Example
/// ```
/// use arrow_buffer::Buffer;
/// use arrow_data::ArrayData;
/// use arrow_data::transform::MutableArrayData;
/// use arrow_schema::DataType;
/// fn i32_array(values: &[i32]) -> ArrayData {
///   ArrayData::try_new(DataType::Int32, values.len(), None, 0, vec![Buffer::from_slice_ref(values)], vec![]).unwrap()
/// }
/// let arr1  = i32_array(&[1, 2, 3, 4, 5]);
/// let arr2  = i32_array(&[6, 7, 8, 9, 10]);
/// // Create a mutable array for copying values from arr1 and arr2, with a capacity for 6 elements
/// let capacity = 3 * std::mem::size_of::<i32>();
/// let mut mutable = MutableArrayData::new(vec![&arr1, &arr2], false, 10);
/// // Copy the first 3 elements from arr1
/// mutable.extend(0, 0, 3);
/// // Copy the last 3 elements from arr2
/// mutable.extend(1, 2, 5);
/// // Complete the MutableArrayData into a new ArrayData
/// let frozen = mutable.freeze();
/// assert_eq!(frozen, i32_array(&[1, 2, 3, 8, 9, 10]));
/// ```
pub struct MutableArrayData<'a> {
    /// Input arrays: the data being read FROM.
    ///
    /// Note this is "dead code" because all actual references to the arrays are
    /// stored in closures for extending values and nulls.
    #[allow(dead_code)]
    arrays: Vec<&'a ArrayData>,

    /// In progress output array: The data being written TO
    ///
    /// Note these fields are in a separate struct, [_MutableArrayData], as they
    /// cannot be in [MutableArrayData] itself due to mutability invariants (interior
    /// mutability): [MutableArrayData] contains a function that can only mutate
    /// [_MutableArrayData], not [MutableArrayData] itself
    data: _MutableArrayData<'a>,

    /// The child data of the `Array` in Dictionary arrays.
    ///
    /// This is not stored in `_MutableArrayData` because these values are
    /// constant and only needed at the end, when freezing [_MutableArrayData].
    dictionary: Option<ArrayData>,

    /// Variadic data buffers referenced by views.
    ///
    /// Note this this is not stored in `_MutableArrayData` because these values
    /// are constant and only needed at the end, when freezing
    /// [_MutableArrayData]
    variadic_data_buffers: Vec<Buffer>,

    /// function used to extend output array with values from input arrays.
    ///
    /// This function's lifetime is bound to the input arrays because it reads
    /// values from them.
    extend_values: Vec<Extend<'a>>,

    /// function used to extend the output array with nulls from input arrays.
    ///
    /// This function's lifetime is bound to the input arrays because it reads
    /// nulls from it.
    extend_null_bits: Vec<ExtendNullBits<'a>>,

    /// function used to extend the output array with null elements.
    ///
    /// This function is independent of the arrays and therefore has no lifetime.
    extend_nulls: ExtendNulls,
    resource_owner: arrow_schema::resource::ResourceOwnerHandle,
    allocation_owner: arrow_schema::resource::ResourceOwnerHandle,
    allocation_failure: Option<arrow_schema::resource::ResourceOwnerError>,
}

impl std::fmt::Debug for MutableArrayData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // ignores the closures.
        f.debug_struct("MutableArrayData")
            .field("data", &self.data)
            .finish()
    }
}

/// Builds an extend that adds `offset` to the source primitive
/// Additionally validates that `max` fits into the
/// the underlying primitive returning None if not
fn build_extend_dictionary(
    array: &ArrayData,
    offset: usize,
    max: usize,
) -> Result<Extend<'_>, ArrowError> {
    macro_rules! validate_and_build {
        ($dt: ty) => {{
            // `max` is the merged dictionary length; the largest key index is
            // `max - 1`, so the key type only needs to hold `max - 1` (e.g. 256
            // values use keys 0..=255, which fit in u8).
            let _: $dt = max
                .saturating_sub(1)
                .try_into()
                .map_err(|_| ArrowError::DictionaryKeyOverflowError)?;
            let offset: $dt = offset
                .try_into()
                .map_err(|_| ArrowError::DictionaryKeyOverflowError)?;
            primitive::build_extend_with_offset(array, offset)
        }};
    }
    match array.data_type() {
        DataType::Dictionary(child_data_type, _) => match child_data_type.as_ref() {
            DataType::UInt8 => validate_and_build!(u8),
            DataType::UInt16 => validate_and_build!(u16),
            DataType::UInt32 => validate_and_build!(u32),
            DataType::UInt64 => validate_and_build!(u64),
            DataType::Int8 => validate_and_build!(i8),
            DataType::Int16 => validate_and_build!(i16),
            DataType::Int32 => validate_and_build!(i32),
            DataType::Int64 => validate_and_build!(i64),
            _ => unreachable!(),
        },
        _ => Err(ArrowError::DictionaryKeyOverflowError),
    }
}

/// Builds an extend that adds `buffer_offset` to any buffer indices encountered
fn build_extend_view(array: &ArrayData, buffer_offset: u32) -> Result<Extend<'_>, ArrowError> {
    let views = array.buffer::<u128>(0);
    Ok(resource::boxed(
        move |mutable: &mut _MutableArrayData, _, start: usize, len: usize| {
            resource::reserve(
                &mut mutable.buffer1,
                resource::count_bytes(len, std::mem::size_of::<u128>(), "transform view geometry")?,
                "transform view growth",
            )?;
            mutable
                .buffer1
                .extend(views[start..start + len].iter().map(|v| {
                    let len = *v as u32;
                    if len <= 12 {
                        return *v; // Stored inline
                    }
                    let mut view = ByteView::from(*v);
                    view.buffer_index += buffer_offset;
                    view.into()
                }));
            Ok(())
        },
        "transform view callback",
    )?)
}

fn build_extend(array: &ArrayData) -> Result<Extend<'_>, ArrowError> {
    match array.data_type() {
        DataType::Null => null::build_extend(array),
        DataType::Boolean => boolean::build_extend(array),
        DataType::UInt8 => primitive::build_extend::<u8>(array),
        DataType::UInt16 => primitive::build_extend::<u16>(array),
        DataType::UInt32 => primitive::build_extend::<u32>(array),
        DataType::UInt64 => primitive::build_extend::<u64>(array),
        DataType::Int8 => primitive::build_extend::<i8>(array),
        DataType::Int16 => primitive::build_extend::<i16>(array),
        DataType::Int32 => primitive::build_extend::<i32>(array),
        DataType::Int64 => primitive::build_extend::<i64>(array),
        DataType::Float32 => primitive::build_extend::<f32>(array),
        DataType::Float64 => primitive::build_extend::<f64>(array),
        DataType::Date32 | DataType::Time32(_) | DataType::Interval(IntervalUnit::YearMonth) => {
            primitive::build_extend::<i32>(array)
        }
        DataType::Date64
        | DataType::Time64(_)
        | DataType::Timestamp(_, _)
        | DataType::Duration(_)
        | DataType::Interval(IntervalUnit::DayTime) => primitive::build_extend::<i64>(array),
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            primitive::build_extend::<IntervalMonthDayNano>(array)
        }
        DataType::Decimal32(_, _) => primitive::build_extend::<i32>(array),
        DataType::Decimal64(_, _) => primitive::build_extend::<i64>(array),
        DataType::Decimal128(_, _) => primitive::build_extend::<i128>(array),
        DataType::Decimal256(_, _) => primitive::build_extend::<i256>(array),
        DataType::Utf8 | DataType::Binary => variable_size::build_extend::<i32>(array),
        DataType::LargeUtf8 | DataType::LargeBinary => variable_size::build_extend::<i64>(array),
        DataType::BinaryView | DataType::Utf8View => unreachable!("should use build_extend_view"),
        DataType::Map(_, _) | DataType::List(_) => list::build_extend::<i32>(array),
        DataType::LargeList(_) => list::build_extend::<i64>(array),
        DataType::ListView(_) => list_view::build_extend::<i32>(array),
        DataType::LargeListView(_) => list_view::build_extend::<i64>(array),
        DataType::Dictionary(_, _) => unreachable!("should use build_extend_dictionary"),
        DataType::Struct(_) => structure::build_extend(array),
        DataType::FixedSizeBinary(_) => fixed_binary::build_extend(array),
        DataType::Float16 => primitive::build_extend::<f16>(array),
        DataType::FixedSizeList(_, _) => fixed_size_list::build_extend(array),
        DataType::Union(_, mode) => match mode {
            UnionMode::Sparse => union::build_extend_sparse(array),
            UnionMode::Dense => union::build_extend_dense(array),
        },
        DataType::RunEndEncoded(_, _) => run::build_extend(array),
    }
}

fn build_extend_nulls(data_type: &DataType) -> Result<ExtendNulls, ArrowError> {
    let function: fn(&mut _MutableArrayData, usize) -> Result<(), ArrowError> = match data_type {
        DataType::Null => null::extend_nulls,
        DataType::Boolean => boolean::extend_nulls,
        DataType::UInt8 => primitive::extend_nulls::<u8>,
        DataType::UInt16 => primitive::extend_nulls::<u16>,
        DataType::UInt32 => primitive::extend_nulls::<u32>,
        DataType::UInt64 => primitive::extend_nulls::<u64>,
        DataType::Int8 => primitive::extend_nulls::<i8>,
        DataType::Int16 => primitive::extend_nulls::<i16>,
        DataType::Int32 => primitive::extend_nulls::<i32>,
        DataType::Int64 => primitive::extend_nulls::<i64>,
        DataType::Float32 => primitive::extend_nulls::<f32>,
        DataType::Float64 => primitive::extend_nulls::<f64>,
        DataType::Date32 | DataType::Time32(_) | DataType::Interval(IntervalUnit::YearMonth) => {
            primitive::extend_nulls::<i32>
        }
        DataType::Date64
        | DataType::Time64(_)
        | DataType::Timestamp(_, _)
        | DataType::Duration(_)
        | DataType::Interval(IntervalUnit::DayTime) => primitive::extend_nulls::<i64>,
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            primitive::extend_nulls::<IntervalMonthDayNano>
        }
        DataType::Decimal32(_, _) => primitive::extend_nulls::<i32>,
        DataType::Decimal64(_, _) => primitive::extend_nulls::<i64>,
        DataType::Decimal128(_, _) => primitive::extend_nulls::<i128>,
        DataType::Decimal256(_, _) => primitive::extend_nulls::<i256>,
        DataType::Utf8 | DataType::Binary => variable_size::extend_nulls::<i32>,
        DataType::LargeUtf8 | DataType::LargeBinary => variable_size::extend_nulls::<i64>,
        DataType::BinaryView | DataType::Utf8View => primitive::extend_nulls::<u128>,
        DataType::Map(_, _) | DataType::List(_) => list::extend_nulls::<i32>,
        DataType::LargeList(_) => list::extend_nulls::<i64>,
        DataType::ListView(_) => list_view::extend_nulls::<i32>,
        DataType::LargeListView(_) => list_view::extend_nulls::<i64>,
        DataType::Dictionary(child_data_type, _) => match child_data_type.as_ref() {
            DataType::UInt8 => primitive::extend_nulls::<u8>,
            DataType::UInt16 => primitive::extend_nulls::<u16>,
            DataType::UInt32 => primitive::extend_nulls::<u32>,
            DataType::UInt64 => primitive::extend_nulls::<u64>,
            DataType::Int8 => primitive::extend_nulls::<i8>,
            DataType::Int16 => primitive::extend_nulls::<i16>,
            DataType::Int32 => primitive::extend_nulls::<i32>,
            DataType::Int64 => primitive::extend_nulls::<i64>,
            _ => unreachable!(),
        },
        DataType::Struct(_) => structure::extend_nulls,
        DataType::FixedSizeBinary(_) => fixed_binary::extend_nulls,
        DataType::Float16 => primitive::extend_nulls::<f16>,
        DataType::FixedSizeList(_, _) => fixed_size_list::extend_nulls,
        DataType::Union(_, mode) => match mode {
            UnionMode::Sparse => union::extend_nulls_sparse,
            UnionMode::Dense => union::extend_nulls_dense,
        },
        DataType::RunEndEncoded(_, _) => run::extend_nulls,
    };
    Ok(resource::boxed(function, "transform null callback")?)
}

fn preallocate_offset_and_binary_buffer<Offset: ArrowNativeType + Integer>(
    capacity: usize,
    binary_size: usize,
) -> Result<[MutableBuffer; 2], ArrowError> {
    // offsets
    let count = capacity
        .checked_add(1)
        .ok_or_else(|| resource::overflow("transform offset geometry"))?;
    let mut buffer = resource::buffer(
        resource::count_bytes(count, mem::size_of::<Offset>(), "transform offset geometry")?,
        "transform offset backing",
    )?;
    // safety: `unsafe` code assumes that this buffer is initialized with one element
    buffer.push(Offset::zero());

    Ok([
        buffer,
        resource::buffer(binary_size, "transform binary backing")?,
    ])
}

/// Define capacities to pre-allocate for child data or data buffers.
#[derive(Debug, Clone)]
pub enum Capacities {
    /// Binary, Utf8 and LargeUtf8 data types
    ///
    /// Defines
    /// * the capacity of the array offsets
    /// * the capacity of the binary/ str buffer
    Binary(usize, Option<usize>),
    /// List and LargeList data types
    ///
    /// Defines
    /// * the capacity of the array offsets
    /// * the capacity of the child data
    List(usize, Option<Box<Capacities>>),
    /// Struct type
    ///
    /// Defines
    /// * the capacity of the array
    /// * the capacities of the fields
    Struct(usize, Option<Vec<Capacities>>),
    /// Dictionary type
    ///
    /// Defines
    /// * the capacity of the array/keys
    /// * the capacity of the values
    Dictionary(usize, Option<Box<Capacities>>),
    /// Don't preallocate inner buffers and rely on array growth strategy
    Array(usize),
}

impl<'a> MutableArrayData<'a> {
    /// Returns a new [MutableArrayData] with capacity to `capacity` slots and
    /// specialized to create an [ArrayData] from multiple `arrays`.
    ///
    /// # Arguments
    /// * `arrays` - the source arrays to copy from
    /// * `use_nulls` - a flag indicating whether the caller intends to call `extend_nulls`.
    ///   Note: null-handling is enabled automatically if any source array contains nulls.
    /// * `capacity` - the preallocated capacity of the output array, in slots (number of elements)
    ///
    /// if `use_nulls` is `false` and no source arrays contains nulls, calling
    /// [MutableArrayData::extend_nulls] or [MutableArrayData::try_extend_nulls] will panic.
    pub fn new(arrays: Vec<&'a ArrayData>, use_nulls: bool, capacity: usize) -> Self {
        Self::with_capacities(arrays, use_nulls, Capacities::Array(capacity))
    }

    /// Similar to [MutableArrayData::new], but lets users define the
    /// preallocated capacities of the array with more granularity.
    ///
    /// See [MutableArrayData::new] for more information on the arguments.
    ///
    /// # Panics
    ///
    /// This function panics if the given `capacities` don't match the data type
    /// of `arrays`. Or when a [Capacities] variant is not yet supported.
    pub fn with_capacities(
        arrays: Vec<&'a ArrayData>,
        use_nulls: bool,
        capacities: Capacities,
    ) -> Self {
        let _scope = resource::enter(arrow_schema::resource::ResourceOwnerHandle::empty());
        Self::try_with_capacities_impl(arrays, use_nulls, &capacities)
            .expect("MutableArrayData::with_capacities failed")
    }

    /// Construct a native transform after fallible admission of its allocation
    /// layouts. The source descriptor Vec is admitted internally: callers can
    /// pass a stack slice such as `&[&left, &right]` without allocating first.
    pub fn try_new(
        arrays: &[&'a ArrayData],
        use_nulls: bool,
        capacity: usize,
    ) -> Result<Self, ArrowError> {
        Self::try_with_capacities(arrays, use_nulls, Capacities::Array(capacity))
    }

    /// Fallible capacity-aware constructor. Original owners are adopted before
    /// native allocation and reservations remain attached through freeze.
    pub fn try_with_capacities(
        arrays: &[&'a ArrayData],
        use_nulls: bool,
        capacities: Capacities,
    ) -> Result<Self, ArrowError> {
        let _scope = resource::enter(arrow_schema::resource::ResourceOwnerHandle::capture());
        let mut inputs = resource::vec(arrays.len(), "transform input references")?;
        inputs.extend_from_slice(arrays);
        Self::try_with_capacities_impl(inputs, use_nulls, &capacities)
    }

    fn try_with_capacities_impl(
        arrays: Vec<&'a ArrayData>,
        use_nulls: bool,
        capacities: &Capacities,
    ) -> Result<Self, ArrowError> {
        let mut resource_owner = arrow_schema::resource::ResourceOwnerHandle::capture();
        for input in &arrays {
            resource_owner.try_inherit(input.resource_owner())?;
        }
        let data_type = arrays
            .first()
            .ok_or_else(|| {
                ArrowError::InvalidArgumentError("transform requires input arrays".into())
            })?
            .data_type();

        for a in arrays.iter().skip(1) {
            assert_eq!(
                data_type,
                a.data_type(),
                "Arrays with inconsistent types passed to MutableArrayData"
            )
        }

        // if any of the arrays has nulls, insertions from any array requires setting bits
        // as there is at least one array with nulls.
        let use_nulls = use_nulls | arrays.iter().any(|array| array.null_count() > 0);

        let array_capacity = match capacities {
            Capacities::Array(capacity)
            | Capacities::Binary(capacity, _)
            | Capacities::List(capacity, _)
            | Capacities::Struct(capacity, _) => *capacity,
            Capacities::Dictionary(_, _) => {
                return Err(ArrowError::InvalidArgumentError(
                    "dictionary capacity not yet supported".into(),
                ));
            }
        };
        let [buffer1, buffer2] = match (data_type, capacities) {
            (
                DataType::LargeUtf8 | DataType::LargeBinary,
                Capacities::Binary(capacity, Some(value_cap)),
            ) => preallocate_offset_and_binary_buffer::<i64>(*capacity, *value_cap)?,
            (DataType::Utf8 | DataType::Binary, Capacities::Binary(capacity, Some(value_cap))) => {
                preallocate_offset_and_binary_buffer::<i32>(*capacity, *value_cap)?
            }
            _ => resource::new_buffers(data_type, array_capacity)?,
        };
        let child_count = match data_type {
            DataType::List(_)
            | DataType::LargeList(_)
            | DataType::ListView(_)
            | DataType::LargeListView(_)
            | DataType::Map(_, _)
            | DataType::FixedSizeList(_, _) => 1,
            DataType::Struct(fields) => fields.len(),
            DataType::Union(fields, _) => fields.len(),
            DataType::RunEndEncoded(_, _) => 2,
            _ => 0,
        };
        let mut child_data = resource::vec(child_count, "transform child descriptors")?;
        for index in 0..child_count {
            let mut children = resource::vec(arrays.len(), "transform child input references")?;
            children.extend(arrays.iter().map(|array| &array.child_data()[index]));
            let fallback = match data_type {
                DataType::FixedSizeList(_, size) => Capacities::Array(
                    array_capacity
                        .checked_mul(
                            usize::try_from(*size)
                                .map_err(|_| resource::overflow("negative fixed list size"))?,
                        )
                        .ok_or_else(|| resource::overflow("fixed list child capacity"))?,
                ),
                _ => Capacities::Array(array_capacity),
            };
            let child_capacity = match capacities {
                Capacities::List(_, Some(child)) => child.as_ref(),
                Capacities::Struct(_, Some(children)) => children.get(index).ok_or_else(|| {
                    ArrowError::InvalidArgumentError("missing struct child capacity".into())
                })?,
                _ => &fallback,
            };
            let child_nulls = if matches!(data_type, DataType::RunEndEncoded(_, _)) && index == 0 {
                false
            } else {
                use_nulls
            };
            child_data.push(Self::try_with_capacities_impl(
                children,
                child_nulls,
                child_capacity,
            )?);
        }

        // Get the dictionary if any, and if it is a concatenation of multiple
        let (dictionary, dict_concat) = match &data_type {
            DataType::Dictionary(_, _) => {
                // If more than one dictionary, concatenate dictionaries together
                let dict_concat = !arrays
                    .windows(2)
                    .all(|a| a[0].child_data()[0].ptr_eq(&a[1].child_data()[0]));

                match dict_concat {
                    false => (
                        Some(resource::clone_data(&arrays[0].child_data()[0])?),
                        false,
                    ),
                    true => {
                        let mut dictionaries =
                            resource::vec(arrays.len(), "transform dictionary inputs")?;
                        let mut lengths =
                            resource::vec(arrays.len(), "transform dictionary lengths")?;
                        let mut capacity = 0usize;
                        for array in &arrays {
                            let dictionary = &array.child_data()[0];
                            capacity = capacity.checked_add(dictionary.len()).ok_or_else(|| {
                                resource::overflow("transform dictionary capacity")
                            })?;
                            dictionaries.push(dictionary);
                            lengths.push(dictionary.len());
                        }
                        let mut mutable = Self::try_with_capacities_impl(
                            dictionaries,
                            false,
                            &Capacities::Array(capacity),
                        )?;
                        for (i, len) in lengths.iter().enumerate() {
                            mutable.try_extend(i, 0, *len)?;
                        }
                        (Some(mutable.try_freeze()?), true)
                    }
                }
            }
            _ => (None, false),
        };

        let mut variadic_data_buffers =
            if matches!(data_type, DataType::BinaryView | DataType::Utf8View) {
                let count = arrays.iter().try_fold(0usize, |count, array| {
                    count
                        .checked_add(array.buffers().len().saturating_sub(1))
                        .ok_or_else(|| resource::overflow("transform variadic descriptor count"))
                })?;
                resource::vec(count, "transform variadic buffer descriptors")?
            } else {
                Vec::new()
            };
        if matches!(data_type, DataType::BinaryView | DataType::Utf8View) {
            variadic_data_buffers.extend(
                arrays
                    .iter()
                    .flat_map(|array| array.buffers().iter().skip(1))
                    .cloned(),
            );
        }
        let extend_nulls = build_extend_nulls(data_type)?;
        let mut extend_null_bits =
            resource::vec(arrays.len(), "transform validity callback descriptors")?;
        for array in &arrays {
            extend_null_bits.push(build_extend_null_bits(array, use_nulls)?);
        }
        let null_buffer = if use_nulls {
            let null_bytes = array_capacity / 8 + usize::from(array_capacity % 8 != 0);
            Some(resource::zeroed(null_bytes, "transform initial validity")?)
        } else {
            None
        };
        let mut extend_values =
            resource::vec(arrays.len(), "transform value callback descriptors")?;
        let mut next_dictionary_offset = 0usize;
        let mut next_buffer_offset = 0u32;
        for array in &arrays {
            let extend = match data_type {
                DataType::Dictionary(_, _) => {
                    let offset = next_dictionary_offset;
                    let maximum = offset
                        .checked_add(array.child_data()[0].len())
                        .ok_or_else(|| resource::overflow("transform dictionary offsets"))?;
                    if dict_concat {
                        next_dictionary_offset = maximum;
                    }
                    build_extend_dictionary(array, offset, maximum)?
                }
                DataType::BinaryView | DataType::Utf8View => {
                    let offset = next_buffer_offset;
                    let count = u32::try_from(array.buffers().len() - 1)
                        .map_err(|_| resource::overflow("transform view buffer count"))?;
                    next_buffer_offset = next_buffer_offset
                        .checked_add(count)
                        .ok_or_else(|| resource::overflow("transform view buffer count"))?;
                    build_extend_view(array, offset)?
                }
                _ => build_extend(array)?,
            };
            extend_values.push(extend);
        }

        let data = _MutableArrayData {
            data_type: resource::data_type_clone(data_type)?,
            len: 0,
            null_count: 0,
            null_buffer,
            buffer1,
            buffer2,
            child_data,
        };
        Ok(Self {
            allocation_failure: None,
            allocation_owner: resource::current(),
            arrays,
            data,
            dictionary,
            variadic_data_buffers,
            extend_values,
            extend_null_bits,
            extend_nulls,
            resource_owner,
        })
    }

    /// Extends the in progress array with a region of the input arrays, returning an error on
    /// overflow.
    ///
    /// # Arguments
    /// * `index` - the index of array that you want to copy values from
    /// * `start` - the start index of the chunk (inclusive)
    /// * `end` - the end index of the chunk (exclusive)
    ///
    /// # Errors
    /// Returns an error if offset arithmetic overflows the underlying integer type.
    ///
    /// # Panic
    /// This function panics if there is an invalid index,
    /// i.e. `index` >= the number of source arrays
    /// or `end` > the length of the `index`th array
    pub fn try_extend(&mut self, index: usize, start: usize, end: usize) -> Result<(), ArrowError> {
        resource::validate_owner(&self.allocation_owner)?;
        if let Some(error) = self.allocation_failure {
            return Err(error.into());
        }
        let result = self.try_extend_impl(index, start, end);
        self.record_allocation_failure(&result);
        result
    }

    fn record_allocation_failure(&mut self, result: &Result<(), ArrowError>) {
        if let Err(ArrowError::ResourceOwnerError(error)) = result {
            self.allocation_failure = Some(*error);
            if let Some(owner) = self.allocation_owner.owner() {
                owner.record_failure(*error);
            }
        }
    }

    fn try_extend_impl(
        &mut self,
        index: usize,
        start: usize,
        end: usize,
    ) -> Result<(), ArrowError> {
        let _scope = resource::enter(self.allocation_owner.clone());
        let len = end - start;
        (self.extend_null_bits[index])(&mut self.data, start, len)?;
        // Snapshot buffer lengths before attempting the extend so we can roll
        // back to a consistent state if it fails.
        let buf1_len = self.data.buffer1.len();
        let buf2_len = self.data.buffer2.len();
        if let Err(e) = (self.extend_values[index])(&mut self.data, index, start, len) {
            // Restore buffers to their pre-call lengths so the array remains
            // in a valid state for the caller to inspect or retry.
            self.data.buffer1.truncate(buf1_len);
            self.data.buffer2.truncate(buf2_len);
            return Err(e);
        }
        self.data.len += len;
        Ok(())
    }

    /// Extends the in progress array with a region of the input arrays.
    ///
    /// # Panic
    /// This function panics if there is an invalid index,
    /// i.e. `index` >= the number of source arrays,
    /// `end` > the length of the `index`th array,
    /// or the offset type overflows (e.g. more than 2 GiB in a `StringArray`).
    #[deprecated(
        since = "59.0.0",
        note = "Use `try_extend` which returns an error on overflow instead of panicking"
    )]
    pub fn extend(&mut self, index: usize, start: usize, end: usize) {
        self.try_extend(index, start, end)
            .expect("extend failed due to offset overflow")
    }

    /// Extends the in progress array with null elements, ignoring the input arrays, returning an
    /// error on overflow.
    ///
    /// Prefer this over [`extend_nulls`](Self::extend_nulls) to handle cases where the run-end
    /// counter overflows (relevant for `RunEndEncoded` arrays).
    ///
    /// # Panics
    ///
    /// Panics if [`MutableArrayData`] not created with `use_nulls` or nullable source arrays
    pub fn try_extend_nulls(&mut self, len: usize) -> Result<(), ArrowError> {
        resource::validate_owner(&self.allocation_owner)?;
        if let Some(error) = self.allocation_failure {
            return Err(error.into());
        }
        let result = self.try_extend_nulls_impl(len);
        self.record_allocation_failure(&result);
        result
    }

    fn try_extend_nulls_impl(&mut self, len: usize) -> Result<(), ArrowError> {
        let _scope = resource::enter(self.allocation_owner.clone());
        self.data.len = self
            .data
            .len
            .checked_add(len)
            .ok_or_else(|| resource::overflow("transform null length"))?;
        let bit_len = bit_util::ceil(self.data.len, 8);
        let nulls = self.data.null_buffer();
        resource::resize(nulls, bit_len, 0, "transform null validity growth")?;
        self.data.null_count += len;
        (self.extend_nulls)(&mut self.data, len)?;
        Ok(())
    }

    /// Extends the in progress array with null elements, ignoring the input arrays.
    ///
    /// # Panics
    ///
    /// Panics if [`MutableArrayData`] not created with `use_nulls` or nullable source arrays,
    /// or if the run-end counter overflows for `RunEndEncoded` arrays.
    #[deprecated(
        since = "59.0.0",
        note = "Use `try_extend_nulls` which returns an error on overflow instead of panicking"
    )]
    pub fn extend_nulls(&mut self, len: usize) {
        self.try_extend_nulls(len)
            .expect("extend_nulls failed due to overflow")
    }

    /// Returns the current length
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len
    }

    /// Returns true if len is 0
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.len == 0
    }

    /// Returns the current null count
    #[inline]
    pub fn null_count(&self) -> usize {
        self.data.null_count
    }

    /// Creates a [ArrayData] from the in progress array, consuming `self`.
    pub fn freeze(self) -> ArrayData {
        self.try_freeze().expect("native transform freeze failed")
    }

    /// Fallibly finalize a governed transform, preserving typed admission errors.
    pub fn try_freeze(self) -> Result<ArrayData, ArrowError> {
        let builder = self.try_into_builder()?;
        // SAFETY: use the same native transform validity contract as freeze.
        // Resource failures are terminal and rejected before transferring parts.
        // This preserves the upstream freeze path without a second validation
        // pass allocating transient DataTypeLayout descriptors.
        Ok(unsafe { builder.build_unchecked() })
    }

    /// Consume self and returns the in progress array as [`ArrayDataBuilder`].
    ///
    /// This is useful for extending the default behavior of MutableArrayData.
    pub fn into_builder(self) -> ArrayDataBuilder {
        self.try_into_builder()
            .expect("native transform finalization failed")
    }

    /// Fallibly finalize native descriptor vectors and immutable buffer owners.
    pub fn try_into_builder(self) -> Result<ArrayDataBuilder, ArrowError> {
        resource::validate_owner(&self.allocation_owner)?;
        if let Some(error) = self.allocation_failure {
            return Err(error.into());
        }
        let _scope = resource::enter(self.allocation_owner.clone());
        let mut owner = self.resource_owner;
        owner.try_capture_current()?;
        let data = self.data;
        let count = match data.data_type {
            DataType::Null
            | DataType::Struct(_)
            | DataType::FixedSizeList(_, _)
            | DataType::RunEndEncoded(_, _) => 0,
            DataType::BinaryView | DataType::Utf8View => self
                .variadic_data_buffers
                .len()
                .checked_add(1)
                .ok_or_else(|| resource::overflow("transform view output descriptors"))?,
            DataType::Utf8
            | DataType::Binary
            | DataType::LargeUtf8
            | DataType::LargeBinary
            | DataType::ListView(_)
            | DataType::LargeListView(_)
            | DataType::Union(_, UnionMode::Dense) => 2,
            _ => 1,
        };
        let mut buffers = resource::vec(count, "transform output buffer descriptors")?;
        if count != 0 {
            buffers.push(resource::finish_buffer(data.buffer1)?);
        }
        if matches!(data.data_type, DataType::BinaryView | DataType::Utf8View) {
            buffers.extend(self.variadic_data_buffers);
        } else if count == 2 {
            buffers.push(resource::finish_buffer(data.buffer2)?);
        }
        let child_count = if matches!(data.data_type, DataType::Dictionary(_, _)) {
            1
        } else {
            data.child_data.len()
        };
        let mut children = resource::vec(child_count, "transform output child descriptors")?;
        if matches!(data.data_type, DataType::Dictionary(_, _)) {
            children.push(
                self.dictionary
                    .expect("dictionary transform has dictionary"),
            );
        } else {
            for child in data.child_data {
                children.push(child.try_freeze()?);
            }
        }
        let nulls = if matches!(
            data.data_type,
            DataType::RunEndEncoded(_, _) | DataType::Null | DataType::Union(_, _)
        ) {
            None
        } else if let Some(nulls) = data.null_buffer {
            let bools = BooleanBuffer::new(resource::finish_buffer(nulls)?, 0, data.len);
            // SAFETY: native extension functions maintain the validity count.
            let nulls = unsafe { NullBuffer::new_unchecked(bools, data.null_count) };
            (nulls.null_count() > 0).then_some(nulls)
        } else {
            None
        };
        Ok(
            ArrayDataBuilder::new_with_resource_owner(data.data_type, owner)
                .offset(0)
                .len(data.len)
                .nulls(nulls)
                .buffers(buffers)
                .child_data(children),
        )
    }
}

// See arrow/tests/array_transform.rs for tests of transform functionality

#[cfg(test)]
mod test {
    use super::*;
    use arrow_schema::Field;
    use std::sync::Arc;

    #[test]
    fn test_list_append_with_capacities() {
        let array = ArrayData::new_empty(&DataType::List(Arc::new(Field::new(
            "element",
            DataType::Int64,
            false,
        ))));

        let mutable = MutableArrayData::with_capacities(
            vec![&array],
            false,
            Capacities::List(6, Some(Box::new(Capacities::Array(17)))),
        );

        // capacities are rounded up to multiples of 64 by MutableBuffer
        assert_eq!(mutable.data.buffer1.capacity(), 64);
        assert_eq!(mutable.data.child_data[0].data.buffer1.capacity(), 192);
    }
}
