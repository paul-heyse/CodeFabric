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

use arrow_array::builder::GenericStringBuilder;
use arrow_array::{ArrayRef, GenericStringArray, OffsetSizeTrait};
use arrow_schema::ArrowError;
use itoa;
use ryu;

use crate::reader::tape::{Tape, TapeElement};
use crate::reader::{ArrayDecoder, DecoderContext};

const TRUE: &str = "true";
const FALSE: &str = "false";

pub struct StringArrayDecoder<O: OffsetSizeTrait> {
    coerce_primitive: bool,
    ignore_type_conflicts: bool,
    phantom: PhantomData<O>,
}

impl<O: OffsetSizeTrait> StringArrayDecoder<O> {
    pub fn new(ctx: &DecoderContext) -> Self {
        Self {
            coerce_primitive: ctx.coerce_primitive(),
            ignore_type_conflicts: ctx.ignore_type_conflicts(),
            phantom: Default::default(),
        }
    }
}

impl<O: OffsetSizeTrait> ArrayDecoder for StringArrayDecoder<O> {
    fn decode(&mut self, tape: &Tape<'_>, pos: &[u32]) -> Result<ArrayRef, ArrowError> {
        let coerce_primitive = self.coerce_primitive;

        let mut data_capacity = 0;
        for p in pos {
            match tape.get(*p) {
                TapeElement::String(idx) => {
                    data_capacity = resource::checked_add(
                        data_capacity,
                        tape.get_string(idx).len(),
                        "JSON string backing",
                    )?;
                }
                TapeElement::Null => {}
                TapeElement::True if coerce_primitive => {
                    data_capacity =
                        resource::checked_add(data_capacity, TRUE.len(), "JSON string backing")?;
                }
                TapeElement::False if coerce_primitive => {
                    data_capacity =
                        resource::checked_add(data_capacity, FALSE.len(), "JSON string backing")?;
                }
                TapeElement::Number(idx) if coerce_primitive => {
                    data_capacity = resource::checked_add(
                        data_capacity,
                        tape.get_string(idx).len(),
                        "JSON string backing",
                    )?;
                }
                TapeElement::I64(_)
                | TapeElement::I32(_)
                | TapeElement::F64(_)
                | TapeElement::F32(_)
                    if coerce_primitive =>
                {
                    let n = match tape.get(*p) {
                        TapeElement::I32(v) => itoa::Buffer::new().format(v).len(),
                        TapeElement::I64(high) => match tape.get(p + 1) {
                            TapeElement::I32(low) => itoa::Buffer::new()
                                .format(((high as i64) << 32) | low as u32 as i64)
                                .len(),
                            _ => unreachable!(),
                        },
                        TapeElement::F32(v) => {
                            ryu::Buffer::new().format_finite(f32::from_bits(v)).len()
                        }
                        TapeElement::F64(high) => match tape.get(p + 1) {
                            TapeElement::F32(low) => ryu::Buffer::new()
                                .format_finite(f64::from_bits(((high as u64) << 32) | low as u64))
                                .len(),
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    };
                    data_capacity = resource::checked_add(data_capacity, n, "JSON string backing")?;
                }
                _ if self.ignore_type_conflicts => {}
                _ => {
                    return Err(tape.error(*p, "string"));
                }
            }
        }

        if O::from_usize(data_capacity).is_none() {
            return Err(resource::json_error(format_args!(
                "offset overflow decoding {}",
                GenericStringArray::<O>::DATA_TYPE
            )));
        }

        resource::offsets::<O>(pos.len())?;
        resource::byte_buffer(data_capacity)?;
        resource::nulls(pos.len())?;
        let mut builder = GenericStringBuilder::<O>::with_capacity(pos.len(), data_capacity);

        let mut float_formatter = ryu::Buffer::new();
        let mut int_formatter = itoa::Buffer::new();

        for p in pos {
            match tape.get(*p) {
                TapeElement::String(idx) => {
                    builder.append_value(tape.get_string(idx));
                }
                TapeElement::Null => builder.append_null(),
                TapeElement::True if coerce_primitive => {
                    builder.append_value(TRUE);
                }
                TapeElement::False if coerce_primitive => {
                    builder.append_value(FALSE);
                }
                TapeElement::Number(idx) if coerce_primitive => {
                    builder.append_value(tape.get_string(idx));
                }
                TapeElement::I64(high) if coerce_primitive => match tape.get(p + 1) {
                    TapeElement::I32(low) => {
                        let val = ((high as i64) << 32) | (low as u32) as i64;
                        builder.append_value(int_formatter.format(val));
                    }
                    _ => unreachable!(),
                },
                TapeElement::I32(n) if coerce_primitive => {
                    builder.append_value(int_formatter.format(n));
                }
                TapeElement::F32(n) if coerce_primitive => {
                    builder.append_value(float_formatter.format_finite(f32::from_bits(n)));
                }
                TapeElement::F64(high) if coerce_primitive => match tape.get(p + 1) {
                    TapeElement::F32(low) => {
                        let val = f64::from_bits(((high as u64) << 32) | low as u64);
                        builder.append_value(float_formatter.format_finite(val));
                    }
                    _ => unreachable!(),
                },
                _ if self.ignore_type_conflicts => builder.append_null(),
                _ => unreachable!(),
            }
        }

        resource::finish_metadata(std::mem::size_of::<O>())?;
        resource::array(builder.finish())
    }
}
