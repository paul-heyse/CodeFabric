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

use super::{_MutableArrayData, Extend};
use crate::ArrayData;
use arrow_schema::{ArrowError, DataType};

pub(super) fn build_extend_sparse(
    array: &ArrayData,
) -> Result<Extend<'_>, arrow_schema::ArrowError> {
    let type_ids = array.buffer::<i8>(0);

    Ok(super::resource::boxed(
        move |mutable: &mut _MutableArrayData, index: usize, start: usize, len: usize| {
            super::resource::reserve(&mut mutable.buffer1, len, "transform union type id growth")?;
            // extends type_ids
            mutable
                .buffer1
                .extend_from_slice(&type_ids[start..start + len]);

            for child in mutable.child_data.iter_mut() {
                child.try_extend(index, start, start + len)?;
            }
            Ok(())
        },
        "transform value callback",
    )?)
}

pub(super) fn build_extend_dense(
    array: &ArrayData,
) -> Result<Extend<'_>, arrow_schema::ArrowError> {
    let type_ids = array.buffer::<i8>(0);
    let offsets = array.buffer::<i32>(1);
    let arrow_schema::DataType::Union(src_fields, _) = array.data_type() else {
        unreachable!();
    };

    Ok(super::resource::boxed(
        move |mutable: &mut _MutableArrayData, index: usize, start: usize, len: usize| {
            super::resource::reserve(&mut mutable.buffer1, len, "transform union type id growth")?;
            // extends type_ids
            mutable
                .buffer1
                .extend_from_slice(&type_ids[start..start + len]);

            super::resource::reserve(
                &mut mutable.buffer2,
                super::resource::count_bytes(
                    len,
                    std::mem::size_of::<i32>(),
                    "transform dense union geometry",
                )?,
                "transform dense union offsets",
            )?;
            for i in start..start + len {
                let type_id = type_ids[i];
                let child_index = src_fields
                    .iter()
                    .position(|(r, _)| r == type_id)
                    .expect("invalid union type ID");
                let src_offset = offsets[i] as usize;
                let child_data = &mut mutable.child_data[child_index];
                let dst_offset = child_data.len();

                // Extend offsets
                mutable.buffer2.push(dst_offset as i32);
                mutable.child_data[child_index].try_extend(index, src_offset, src_offset + 1)?;
            }
            Ok(())
        },
        "transform value callback",
    )?)
}

pub(super) fn extend_nulls_dense(
    mutable: &mut _MutableArrayData,
    len: usize,
) -> Result<(), ArrowError> {
    let DataType::Union(fields, _) = &mutable.data_type else {
        unreachable!()
    };
    let first_type_id = fields
        .iter()
        .next()
        .expect("union must have at least one field")
        .0;

    // Extend type_ids buffer
    super::resource::reserve(&mut mutable.buffer1, len, "transform union null type ids")?;
    mutable
        .buffer1
        .extend(std::iter::repeat_n(first_type_id, len));

    // Dense: extend offsets pointing into the first child, then extend nulls in that child
    let child_offset = mutable.child_data[0].len();
    let (start, end) = (child_offset as i32, (child_offset + len) as i32);
    super::resource::reserve(
        &mut mutable.buffer2,
        super::resource::count_bytes(
            len,
            std::mem::size_of::<i32>(),
            "transform dense union null geometry",
        )?,
        "transform dense union null offsets",
    )?;
    mutable.buffer2.extend(start..end);
    mutable.child_data[0].try_extend_nulls(len)?;
    Ok(())
}

pub(super) fn extend_nulls_sparse(
    mutable: &mut _MutableArrayData,
    len: usize,
) -> Result<(), ArrowError> {
    let DataType::Union(fields, _) = &mutable.data_type else {
        unreachable!()
    };
    let first_type_id = fields
        .iter()
        .next()
        .expect("union must have at least one field")
        .0;

    // Extend type_ids buffer
    super::resource::reserve(&mut mutable.buffer1, len, "transform union null type ids")?;
    mutable
        .buffer1
        .extend(std::iter::repeat_n(first_type_id, len));

    // Sparse: extend nulls in ALL children
    for child in mutable.child_data.iter_mut() {
        child.try_extend_nulls(len)?;
    }
    Ok(())
}
