// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information.
// The ASF licenses this file to you under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Admission for the existing native ArrayData conversions and array factory.
//! These bounds cover descriptor allocations, not original input backing.

use crate::types::*;
use crate::*;
use arrow_buffer::Buffer;
use arrow_data::ArrayData;
use arrow_schema::resource::{
    ResourceAllocationRequest, ResourceOwnerError, ResourceOwnerHandle, RetainedResourceOwner,
    current_resource_owner, enter_resource_owner,
};
use arrow_schema::{ArrowError, DataType, IntervalUnit, TimeUnit, UnionMode};
use std::alloc::Layout;
use std::sync::Arc;

type Owner = Arc<dyn RetainedResourceOwner>;
type Result<T> = std::result::Result<T, ArrowError>;

fn error(kind: &'static str) -> ArrowError {
    ResourceOwnerError {
        kind,
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
    .into()
}
fn plus(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| error("native array descriptor overflow"))
}
fn times(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| error("native array descriptor overflow"))
}
pub(crate) fn admit(owner: &Owner, layout: Layout, kind: &'static str) -> Result<()> {
    if layout.size() == 0 {
        return Ok(());
    }
    owner
        .try_reserve_allocation(ResourceAllocationRequest {
            bytes: layout.size(),
            alignment: layout.align(),
            kind,
        })
        .map_err(|e| {
            owner.record_failure(e);
            e.into()
        })
}
pub(crate) fn vector<T>(owner: &Owner, count: usize, kind: &'static str) -> Result<()> {
    let layout = Layout::array::<T>(count).map_err(|_| error(kind))?;
    admit(owner, layout, kind)
}
pub(crate) fn arc_slice<T>(owner: &Owner, count: usize, kind: &'static str) -> Result<()> {
    // Rust 1.98 ArcInner is repr(C, align(2)): strong/weak usize counters,
    // followed by the concrete payload. Both the header and tail are admitted.
    let tail = Layout::array::<T>(count).map_err(|_| error(kind))?;
    let (layout, _) = Layout::new::<[usize; 2]>()
        .extend(tail)
        .map_err(|_| error(kind))?;
    admit(owner, layout.pad_to_align(), kind)
}
pub(crate) fn array_arc<T>(owner: &Owner) -> Result<()> {
    arc_slice::<T>(owner, 1, "native concrete array Arc")
}
fn inherit(owner: &Owner, original: &ResourceOwnerHandle) -> Result<()> {
    let Some(original) = original.owner() else {
        let e = ResourceOwnerError {
            kind: "unowned native array input",
            requested: 1,
            limit: 0,
        };
        owner.record_failure(e);
        return Err(e.into());
    };
    if !Arc::ptr_eq(owner, original) {
        owner.try_adopt(original.clone()).map_err(|e| {
            owner.record_failure(e);
            ArrowError::from(e)
        })?;
    }
    Ok(())
}
fn effective(original: &ResourceOwnerHandle) -> Option<Owner> {
    current_resource_owner().or_else(|| original.owner().cloned())
}
pub(crate) fn data_type_clone(owner: &Owner, kind: &DataType) -> Result<()> {
    if let DataType::Dictionary(key, value) = kind {
        admit(
            owner,
            Layout::new::<DataType>(),
            "native dictionary key type Box",
        )?;
        data_type_clone(owner, key)?;
        admit(
            owner,
            Layout::new::<DataType>(),
            "native dictionary value type Box",
        )?;
        data_type_clone(owner, value)?;
    }
    // Other DataTypes are inline or clone retained Field/Fields/UnionFields/str Arcs.
    Ok(())
}
fn data_clone(owner: &Owner, data: &ArrayData) -> Result<()> {
    inherit(owner, data.resource_owner())?;
    data_type_clone(owner, data.data_type())?;
    vector::<Buffer>(
        owner,
        data.buffers().len(),
        "native sliced ArrayData buffers",
    )?;
    vector::<ArrayData>(
        owner,
        data.child_data().len(),
        "native sliced ArrayData children",
    )?;
    for child in data.child_data() {
        data_clone(owner, child)?;
    }
    Ok(())
}

/// Sealed to native concrete families; custom Array implementations use the
/// trait's rejection default until they implement an explicit admission seam.
pub(crate) trait Geometry: Array {
    fn data_descriptors(&self, owner: &Owner) -> Result<()>;
}
pub(crate) fn prepare_data<T: Geometry>(array: &T) -> Result<()> {
    let Some(owner) = effective(array.resource_owner()) else {
        return Ok(());
    };
    inherit(&owner, array.resource_owner())?;
    let _guard = enter_resource_owner(owner.clone());
    array.data_descriptors(&owner)
}
pub(crate) fn to_data<T: Geometry>(array: &T) -> Result<ArrayData> {
    let Some(owner) = effective(array.resource_owner()) else {
        return Ok(array.to_data());
    };
    let _guard = enter_resource_owner(owner);
    prepare_data(array)?;
    Ok(array.to_data())
}
pub(crate) fn custom_to_data<T: Array + ?Sized>(array: &T) -> Result<ArrayData> {
    if let Some(owner) = effective(array.resource_owner()) {
        let e = ResourceOwnerError {
            kind: "custom array descriptor admission unavailable",
            requested: 1,
            limit: 0,
        };
        owner.record_failure(e);
        Err(e.into())
    } else {
        Ok(array.to_data())
    }
}
pub(crate) fn custom_prepare<T: Array + ?Sized>(array: &T) -> Result<()> {
    if let Some(owner) = effective(array.resource_owner()) {
        let e = ResourceOwnerError {
            kind: "custom array descriptor admission unavailable",
            requested: 1,
            limit: 0,
        };
        owner.record_failure(e);
        Err(e.into())
    } else {
        Ok(())
    }
}

macro_rules! simple {
    ($ty:ty, $buffers:expr) => {
        impl Geometry for $ty {
            fn data_descriptors(&self, owner: &Owner) -> Result<()> {
                data_type_clone(owner, self.data_type())?;
                vector::<Buffer>(owner, $buffers, "native ArrayData buffer descriptors")
            }
        }
    };
}
simple!(BooleanArray, 1);
simple!(NullArray, 0);
simple!(FixedSizeBinaryArray, 1);
impl<T: ArrowPrimitiveType> Geometry for PrimitiveArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        data_type_clone(owner, self.data_type())?;
        vector::<Buffer>(owner, 1, "native ArrayData buffer descriptors")
    }
}
impl<T: ByteArrayType> Geometry for GenericByteArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<Buffer>(owner, 2, "native ArrayData byte descriptors")
    }
}
impl<T: ByteViewType + ?Sized> Geometry for GenericByteViewArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<Buffer>(
            owner,
            plus(self.data_buffers().len(), 1)?,
            "native ArrayData view descriptors",
        )
    }
}
impl<T: OffsetSizeTrait> Geometry for GenericListArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<Buffer>(owner, 1, "native ArrayData list offsets")?;
        vector::<ArrayData>(owner, 1, "native ArrayData list child")?;
        self.values().try_admit_to_data()
    }
}
impl<T: OffsetSizeTrait> Geometry for GenericListViewArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<Buffer>(owner, 2, "native ArrayData list view descriptors")?;
        vector::<ArrayData>(owner, 1, "native ArrayData list view child")?;
        self.values().try_admit_to_data()
    }
}
impl Geometry for FixedSizeListArray {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<ArrayData>(owner, 1, "native ArrayData fixed list child")?;
        self.values().try_admit_to_data()
    }
}
impl Geometry for StructArray {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<ArrayRef>(
            owner,
            self.num_columns(),
            "native StructArray clone columns",
        )?;
        vector::<ArrayData>(
            owner,
            self.num_columns(),
            "native ArrayData struct children",
        )?;
        for child in self.columns() {
            child.try_admit_to_data()?;
        }
        Ok(())
    }
}
impl Geometry for MapArray {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        // Map::clone clones its embedded StructArray; entries.to_data clones it again.
        vector::<ArrayRef>(
            owner,
            self.entries().num_columns(),
            "native MapArray clone entries",
        )?;
        vector::<Buffer>(owner, 1, "native ArrayData map offsets")?;
        vector::<ArrayData>(owner, 1, "native ArrayData map entries")?;
        self.entries().try_admit_to_data()
    }
}
impl Geometry for UnionArray {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        let DataType::Union(fields, mode) = self.data_type() else {
            unreachable!()
        };
        let slots = plus(
            fields
                .iter()
                .map(|(id, _)| id as usize)
                .max()
                .unwrap_or_default(),
            1,
        )?;
        vector::<Option<ArrayRef>>(owner, slots, "native UnionArray clone fields")?;
        vector::<Buffer>(
            owner,
            if *mode == UnionMode::Dense { 2 } else { 1 },
            "native ArrayData union buffers",
        )?;
        vector::<ArrayData>(owner, fields.len(), "native ArrayData union children")?;
        for (id, _) in fields.iter() {
            self.child(id).try_admit_to_data()?;
        }
        Ok(())
    }
}
impl<T: ArrowDictionaryKeyType> Geometry for DictionaryArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        data_type_clone(owner, self.data_type())?;
        vector::<Buffer>(owner, 1, "native ArrayData dictionary keys")?;
        vector::<ArrayData>(owner, 1, "native ArrayData dictionary child")?;
        self.values().try_admit_to_data()
    }
}
impl<T: RunEndIndexType> Geometry for RunArray<T> {
    fn data_descriptors(&self, owner: &Owner) -> Result<()> {
        vector::<Buffer>(owner, 1, "native ArrayData run ends")?;
        vector::<ArrayData>(owner, 2, "native ArrayData run children")?;
        self.values().try_admit_to_data()
    }
}

pub(crate) fn prepare_make(data: &ArrayData) -> Result<()> {
    let Some(owner) = effective(data.resource_owner()) else {
        return Ok(());
    };
    let _guard = enter_resource_owner(owner.clone());
    factory(&owner, data, true, None)
}

pub(crate) fn make(data: ArrayData) -> Result<ArrayRef> {
    let Some(owner) = effective(data.resource_owner()) else {
        return Ok(make_array(data));
    };
    let _guard = enter_resource_owner(owner.clone());
    factory(&owner, &data, true, None)?;
    Ok(make_array(data))
}

fn slice_bounds(data: &ArrayData, offset: usize, len: usize) -> Result<()> {
    if plus(offset, len)? > data.len() {
        return Err(error("native ArrayData slice bounds"));
    }
    if matches!(data.data_type(), DataType::Struct(_)) {
        for child in data.child_data() {
            slice_bounds(child, offset, len)?;
        }
    }
    Ok(())
}

// `sliced` models ArrayData::slice's offset/length without allocating a copy.
fn factory(
    owner: &Owner,
    data: &ArrayData,
    outer_arc: bool,
    sliced: Option<(usize, usize)>,
) -> Result<()> {
    inherit(owner, data.resource_owner())?;
    if let Some((offset, len)) = sliced {
        slice_bounds(data, offset, len)?;
    }
    if outer_arc {
        concrete_arc(owner, data.data_type())?;
    }
    let (delta, len) = sliced.unwrap_or((0, data.len()));
    let offset = plus(data.offset(), delta)?;
    match data.data_type() {
        DataType::Struct(_) => {
            // The native constructor uses one explicit capacity Vec, avoiding
            // capacity-dependent reallocations from in-place iterator collect.
            vector::<ArrayRef>(
                owner,
                data.child_data().len(),
                "native StructArray child descriptors",
            )?;
            for child in data.child_data() {
                let inherited = sliced;
                let child_len = inherited.map_or(child.len(), |(_, len)| len);
                let next = if offset != 0 || len != child_len {
                    data_clone(owner, child)?;
                    Some((
                        plus(inherited.map_or(0, |(offset, _)| offset), offset)?,
                        len,
                    ))
                } else {
                    inherited
                };
                factory(owner, child, true, next)?;
            }
        }
        DataType::FixedSizeList(_, size) => {
            let size =
                usize::try_from(*size).map_err(|_| error("invalid native fixed list size"))?;
            let child = &data.child_data()[0];
            data_clone(owner, child)?;
            factory(
                owner,
                child,
                true,
                Some((times(offset, size)?, times(len, size)?)),
            )?;
        }
        DataType::Map(_, _) => factory(owner, &data.child_data()[0], false, None)?,
        DataType::RunEndEncoded(_, _) => {
            inherit(owner, data.child_data()[0].resource_owner())?;
            factory(owner, &data.child_data()[1], true, None)?;
        }
        DataType::Dictionary(_, _) => {
            vector::<Buffer>(owner, 1, "native dictionary key ArrayData descriptor")?;
            factory(owner, &data.child_data()[0], true, None)?;
        }
        DataType::Utf8View | DataType::BinaryView => {
            let count = data
                .buffers()
                .len()
                .checked_sub(1)
                .ok_or_else(|| error("invalid native view buffer count"))?;
            // Arc::from_iter(Vec::IntoIter) is TrustedLen in Rust 1.98 and
            // allocates its Arc slice directly, including for a zero-length tail.
            arc_slice::<Buffer>(owner, count, "native byte view buffer Arc slice")?;
        }
        DataType::Union(fields, _) => {
            let slots = plus(
                fields
                    .iter()
                    .map(|(id, _)| id as usize)
                    .max()
                    .unwrap_or_default(),
                1,
            )?;
            vector::<Option<ArrayRef>>(owner, slots, "native union child descriptors")?;
            for child in data.child_data() {
                factory(owner, child, true, None)?;
            }
        }
        _ => {
            for child in data.child_data() {
                factory(owner, child, true, None)?;
            }
        }
    }
    Ok(())
}

fn concrete_arc(owner: &Owner, data_type: &DataType) -> Result<()> {
    match data_type {
        DataType::Boolean => array_arc::<BooleanArray>(owner),
        DataType::Int8 => array_arc::<Int8Array>(owner),
        DataType::Int16 => array_arc::<Int16Array>(owner),
        DataType::Int32 => array_arc::<Int32Array>(owner),
        DataType::Int64 => array_arc::<Int64Array>(owner),
        DataType::UInt8 => array_arc::<UInt8Array>(owner),
        DataType::UInt16 => array_arc::<UInt16Array>(owner),
        DataType::UInt32 => array_arc::<UInt32Array>(owner),
        DataType::UInt64 => array_arc::<UInt64Array>(owner),
        DataType::Float16 => array_arc::<Float16Array>(owner),
        DataType::Float32 => array_arc::<Float32Array>(owner),
        DataType::Float64 => array_arc::<Float64Array>(owner),
        DataType::Date32 => array_arc::<Date32Array>(owner),
        DataType::Date64 => array_arc::<Date64Array>(owner),
        DataType::Time32(TimeUnit::Second) => array_arc::<Time32SecondArray>(owner),
        DataType::Time32(TimeUnit::Millisecond) => array_arc::<Time32MillisecondArray>(owner),
        DataType::Time64(TimeUnit::Microsecond) => array_arc::<Time64MicrosecondArray>(owner),
        DataType::Time64(TimeUnit::Nanosecond) => array_arc::<Time64NanosecondArray>(owner),
        DataType::Timestamp(TimeUnit::Second, _) => array_arc::<TimestampSecondArray>(owner),
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            array_arc::<TimestampMillisecondArray>(owner)
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            array_arc::<TimestampMicrosecondArray>(owner)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            array_arc::<TimestampNanosecondArray>(owner)
        }
        DataType::Interval(IntervalUnit::YearMonth) => array_arc::<IntervalYearMonthArray>(owner),
        DataType::Interval(IntervalUnit::DayTime) => array_arc::<IntervalDayTimeArray>(owner),
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            array_arc::<IntervalMonthDayNanoArray>(owner)
        }
        DataType::Duration(TimeUnit::Second) => array_arc::<DurationSecondArray>(owner),
        DataType::Duration(TimeUnit::Millisecond) => array_arc::<DurationMillisecondArray>(owner),
        DataType::Duration(TimeUnit::Microsecond) => array_arc::<DurationMicrosecondArray>(owner),
        DataType::Duration(TimeUnit::Nanosecond) => array_arc::<DurationNanosecondArray>(owner),
        DataType::Binary => array_arc::<BinaryArray>(owner),
        DataType::LargeBinary => array_arc::<LargeBinaryArray>(owner),
        DataType::FixedSizeBinary(_) => array_arc::<FixedSizeBinaryArray>(owner),
        DataType::BinaryView => array_arc::<BinaryViewArray>(owner),
        DataType::Utf8 => array_arc::<StringArray>(owner),
        DataType::LargeUtf8 => array_arc::<LargeStringArray>(owner),
        DataType::Utf8View => array_arc::<StringViewArray>(owner),
        DataType::List(_) => array_arc::<ListArray>(owner),
        DataType::LargeList(_) => array_arc::<LargeListArray>(owner),
        DataType::ListView(_) => array_arc::<ListViewArray>(owner),
        DataType::LargeListView(_) => array_arc::<LargeListViewArray>(owner),
        DataType::Struct(_) => array_arc::<StructArray>(owner),
        DataType::Map(_, _) => array_arc::<MapArray>(owner),
        DataType::Union(_, _) => array_arc::<UnionArray>(owner),
        DataType::FixedSizeList(_, _) => array_arc::<FixedSizeListArray>(owner),
        DataType::Dictionary(key_type, _) => match key_type.as_ref() {
            DataType::Int8 => array_arc::<DictionaryArray<Int8Type>>(owner),
            DataType::Int16 => array_arc::<DictionaryArray<Int16Type>>(owner),
            DataType::Int32 => array_arc::<DictionaryArray<Int32Type>>(owner),
            DataType::Int64 => array_arc::<DictionaryArray<Int64Type>>(owner),
            DataType::UInt8 => array_arc::<DictionaryArray<UInt8Type>>(owner),
            DataType::UInt16 => array_arc::<DictionaryArray<UInt16Type>>(owner),
            DataType::UInt32 => array_arc::<DictionaryArray<UInt32Type>>(owner),
            DataType::UInt64 => array_arc::<DictionaryArray<UInt64Type>>(owner),
            _ => Err(error("unsupported native array descriptor type")),
        },
        DataType::RunEndEncoded(run_ends_type, _) => match run_ends_type.data_type() {
            DataType::Int16 => array_arc::<RunArray<Int16Type>>(owner),
            DataType::Int32 => array_arc::<RunArray<Int32Type>>(owner),
            DataType::Int64 => array_arc::<RunArray<Int64Type>>(owner),
            _ => Err(error("unsupported native array descriptor type")),
        },
        DataType::Null => array_arc::<NullArray>(owner),
        DataType::Decimal32(_, _) => array_arc::<Decimal32Array>(owner),
        DataType::Decimal64(_, _) => array_arc::<Decimal64Array>(owner),
        DataType::Decimal128(_, _) => array_arc::<Decimal128Array>(owner),
        DataType::Decimal256(_, _) => array_arc::<Decimal256Array>(owner),
        _ => Err(error("unsupported native array descriptor type")),
    }
}

/// Finishing a native buffer moves preadmitted payload into one immutable owner.
pub(crate) fn finish_buffer(owner: &Owner) -> Result<()> {
    admit(
        owner,
        arrow_buffer::allocation_owner_layout(),
        "native finished buffer owner",
    )
}
pub(crate) fn finish_nulls(owner: &Owner, materialized: bool) -> Result<()> {
    if materialized {
        finish_buffer(owner)?;
    }
    Ok(())
}
pub(crate) fn field(owner: &Owner, name: &str, data_type: &DataType) -> Result<()> {
    vector::<u8>(owner, name.len(), "native finished field name")?;
    data_type_clone(owner, data_type)?;
    arc_slice::<arrow_schema::Field>(owner, 1, "native finished Field Arc")
}
pub(crate) fn custom_finish<T: crate::builder::ArrayBuilder + ?Sized>(
    builder: &mut T,
) -> Result<ArrayRef> {
    if let Some(owner) = current_resource_owner() {
        let error = ResourceOwnerError {
            kind: "custom builder finalization admission unavailable",
            requested: 1,
            limit: 0,
        };
        owner.record_failure(error);
        Err(error.into())
    } else {
        Ok(builder.finish())
    }
}
