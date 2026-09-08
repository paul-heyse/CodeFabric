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

use crate::resource;
use std::marker::PhantomData;

use arrow_array::{
    ArrayRef, FixedSizeListArray, GenericListArray, GenericListViewArray, OffsetSizeTrait,
};
use arrow_buffer::{NullBufferBuilder, OffsetBuffer, ScalarBuffer};
use arrow_schema::{ArrowError, DataType, FieldRef};

use crate::reader::tape::{Tape, TapeElement};
use crate::reader::{ArrayDecoder, DecoderContext};

pub type ListArrayDecoder<O> = ListLikeArrayDecoder<O, false>;
pub type ListViewArrayDecoder<O> = ListLikeArrayDecoder<O, true>;

pub struct ListLikeArrayDecoder<O, const IS_VIEW: bool> {
    field: FieldRef,
    decoder: Box<dyn ArrayDecoder>,
    phantom: PhantomData<O>,
    ignore_type_conflicts: bool,
    is_nullable: bool,
}

impl<O: OffsetSizeTrait, const IS_VIEW: bool> ListLikeArrayDecoder<O, IS_VIEW> {
    pub fn new(
        ctx: &DecoderContext,
        data_type: &DataType,
        is_nullable: bool,
    ) -> Result<Self, ArrowError> {
        let field = match (IS_VIEW, data_type) {
            (false, DataType::List(f)) if !O::IS_LARGE => f,
            (false, DataType::LargeList(f)) if O::IS_LARGE => f,
            (true, DataType::ListView(f)) if !O::IS_LARGE => f,
            (true, DataType::LargeListView(f)) if O::IS_LARGE => f,
            _ => unreachable!(),
        };
        let decoder = ctx.make_decoder(field.data_type(), field.is_nullable())?;

        Ok(Self {
            field: field.clone(),
            decoder,
            phantom: Default::default(),
            ignore_type_conflicts: ctx.ignore_type_conflicts(),
            is_nullable,
        })
    }
}

impl<O: OffsetSizeTrait, const IS_VIEW: bool> ArrayDecoder for ListLikeArrayDecoder<O, IS_VIEW> {
    fn decode(&mut self, tape: &Tape<'_>, pos: &[u32]) -> Result<ArrayRef, ArrowError> {
        let mut child_pos = resource::new_vec(pos.len(), "JSON child positions")?;
        let mut offsets = resource::new_vec(
            resource::checked_add(pos.len(), 1, "JSON offsets")?,
            "JSON offsets",
        )?;
        offsets.push(O::from_usize(0).unwrap());

        if self.is_nullable {
            resource::nulls(pos.len())?;
        }
        let mut nulls = self.is_nullable.then(|| NullBufferBuilder::new(pos.len()));

        for p in pos {
            let end_idx = match (tape.get(*p), nulls.as_mut()) {
                (TapeElement::StartList(end_idx), None) => end_idx,
                (TapeElement::StartList(end_idx), Some(nulls)) => {
                    nulls.append_non_null();
                    end_idx
                }
                (TapeElement::Null, Some(nulls)) => {
                    nulls.append_null();
                    *p + 1
                }
                (_, Some(nulls)) if self.ignore_type_conflicts => {
                    nulls.append_null();
                    *p + 1
                }
                _ => return Err(tape.error(*p, "[")),
            };

            let mut cur_idx = *p + 1;
            while cur_idx < end_idx {
                resource::reserve_vec(&mut child_pos, 1, "JSON child positions")?;
                child_pos.push(cur_idx);

                // Advance to next field
                cur_idx = tape.next(cur_idx, "list value")?;
            }

            let offset = O::from_usize(child_pos.len()).ok_or_else(|| {
                resource::json_error(format_args!(
                    "offset overflow decoding {}ListArray",
                    O::PREFIX
                ))
            })?;
            offsets.push(offset);
        }

        let values = self.decoder.decode(tape, &child_pos)?;
        resource::collection_error_capacity(self.field.name())?;
        let nulls = nulls.as_mut().and_then(|x| x.finish());

        if IS_VIEW {
            let mut sizes = resource::new_vec(offsets.len() - 1, "JSON list sizes")?;
            for i in 1..offsets.len() {
                sizes.push(offsets[i] - offsets[i - 1]);
            }
            offsets.pop();
            let array = GenericListViewArray::<O>::try_new(
                self.field.clone(),
                ScalarBuffer::from(offsets),
                ScalarBuffer::from(sizes),
                values,
                nulls,
            )?;
            resource::array(array)
        } else {
            // SAFETY: offsets are built monotonically starting from 0
            let offsets = unsafe { OffsetBuffer::<O>::new_unchecked(ScalarBuffer::from(offsets)) };

            let array = GenericListArray::<O>::try_new(self.field.clone(), offsets, values, nulls)?;
            resource::array(array)
        }
    }
}

pub struct FixedSizeListArrayDecoder {
    field: FieldRef,
    size: i32,
    decoder: Box<dyn ArrayDecoder>,
    ignore_type_conflicts: bool,
    is_nullable: bool,
}

impl FixedSizeListArrayDecoder {
    pub fn new(
        ctx: &DecoderContext,
        data_type: &DataType,
        is_nullable: bool,
    ) -> Result<Self, ArrowError> {
        let (field, size) = match data_type {
            DataType::FixedSizeList(f, s) => (f, *s),
            _ => unreachable!(),
        };
        let decoder = ctx.make_decoder(field.data_type(), field.is_nullable())?;

        Ok(Self {
            field: field.clone(),
            size,
            decoder,
            ignore_type_conflicts: ctx.ignore_type_conflicts(),
            is_nullable,
        })
    }
}

impl ArrayDecoder for FixedSizeListArrayDecoder {
    fn decode(&mut self, tape: &Tape<'_>, pos: &[u32]) -> Result<ArrayRef, ArrowError> {
        let expected = self.size as usize;
        let mut child_pos = resource::new_vec(
            resource::checked_mul(pos.len(), expected, "JSON fixed list positions")?,
            "JSON child positions",
        )?;

        if self.is_nullable {
            resource::nulls(pos.len())?;
        }
        let mut nulls = self.is_nullable.then(|| NullBufferBuilder::new(pos.len()));

        for p in pos {
            let end_idx = match (tape.get(*p), nulls.as_mut()) {
                (TapeElement::StartList(end_idx), None) => end_idx,
                (TapeElement::StartList(end_idx), Some(nulls)) => {
                    nulls.append_non_null();
                    end_idx
                }
                (TapeElement::Null, Some(nulls)) => {
                    nulls.append_null();
                    child_pos.resize(child_pos.len() + expected, 0);
                    continue;
                }
                (_, Some(nulls)) if self.ignore_type_conflicts => {
                    nulls.append_null();
                    child_pos.resize(child_pos.len() + expected, 0);
                    continue;
                }
                _ => return Err(tape.error(*p, "[")),
            };

            let child_start = child_pos.len();
            let mut cur_idx = *p + 1;
            while cur_idx < end_idx {
                resource::reserve_vec(&mut child_pos, 1, "JSON child positions")?;
                child_pos.push(cur_idx);
                cur_idx = tape.next(cur_idx, "fixed-size list value")?;
            }

            let actual = child_pos.len() - child_start;
            if actual != expected {
                return Err(resource::json_error(format_args!(
                    "Incorrect number of elements for FixedSizeList, \
                     expected {expected} but got {actual}"
                )));
            }
        }

        let values = self.decoder.decode(tape, &child_pos)?;
        resource::collection_error_capacity(self.field.name())?;
        let nulls = nulls.as_mut().and_then(|x| x.finish());

        // FixedSizeList's native validator obtains logical nulls, and may expand
        // a nullable parent's bitmap to the child length before checking containment.
        if matches!(values.data_type(), DataType::Null) {
            resource::nulls(values.len())?;
        }
        if !self.field.is_nullable() && nulls.is_some() {
            resource::nulls(values.len())?;
        }
        let array = FixedSizeListArray::try_new_with_length(
            self.field.clone(),
            self.size,
            values,
            nulls,
            pos.len(),
        )?;
        resource::array(array)
    }
}
