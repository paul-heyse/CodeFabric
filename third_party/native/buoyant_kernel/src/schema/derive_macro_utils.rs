//! Utility traits that support the [`delta_kernel_derive::ToSchema`] macro.
///
/// Not intended for use by normal code.
use std::collections::{HashMap, HashSet};

use delta_kernel_derive::internal_api;

use crate::schema::{ArrayType, DataType, MapType, StructField, ToSchema};

/// Converts a type to a [`DataType`]. Implemented for the primitive types and automatically derived
/// for all types that implement [`ToSchema`].
#[internal_api]
pub(crate) trait ToDataType {
    fn to_data_type() -> DataType;
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> {
        Err(crate::resource::ResourceExhausted {kind: "native_derived_type_layout", requested: 1, limit: 0}.into())
    }
}

// Blanket impl for all types that implement `ToSchema`
impl<T: ToSchema> ToDataType for T {
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> { add(T::resource_schema_bytes()?, std::mem::size_of::<crate::schema::StructType>()) }

    fn to_data_type() -> DataType {
        T::to_schema().into()
    }
}

// Helper macro to implement `ToDataType` for primitive types
macro_rules! impl_to_data_type {
    ( $(($rust_type: ty, $data_type: expr)), * ) => {
        $(
            impl ToDataType for $rust_type {
                fn resource_data_type_bytes() -> crate::DeltaResult<usize> { Ok(0) }
                fn to_data_type() -> DataType {
                    $data_type
                }
            }
        )*
    };
}

impl_to_data_type!(
    (String, DataType::STRING),
    (i64, DataType::LONG),
    (i32, DataType::INTEGER),
    (i16, DataType::SHORT),
    (char, DataType::BYTE),
    (f32, DataType::FLOAT),
    (f64, DataType::DOUBLE),
    (bool, DataType::BOOLEAN)
);

// ToDataType impl for non-nullable array types
impl<T: ToDataType> ToDataType for Vec<T> {
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> {add(T::resource_data_type_bytes()?, 5 + std::mem::size_of::<ArrayType>())}

    fn to_data_type() -> DataType {
        ArrayType::new(T::to_data_type(), false).into()
    }
}

// ToDataType impl for non-nullable set types
impl<T: ToDataType> ToDataType for HashSet<T> {
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> {add(T::resource_data_type_bytes()?, 5 + std::mem::size_of::<ArrayType>())}

    fn to_data_type() -> DataType {
        ArrayType::new(T::to_data_type(), false).into()
    }
}

// ToDataType impl for non-nullable map types
impl<K: ToDataType, V: ToDataType> ToDataType for HashMap<K, V> {
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> {add(add(K::resource_data_type_bytes()?,V::resource_data_type_bytes()?)?,3+std::mem::size_of::<MapType>())}

    fn to_data_type() -> DataType {
        MapType::new(K::to_data_type(), V::to_data_type(), false).into()
    }
}

// ToDataType impl for maps with nullable values
impl<K: ToDataType, V: ToDataType> ToDataType for HashMap<K, Option<V>> {
    fn resource_data_type_bytes() -> crate::DeltaResult<usize> {add(add(K::resource_data_type_bytes()?,V::resource_data_type_bytes()?)?,3+std::mem::size_of::<MapType>())}

    fn to_data_type() -> DataType {
        MapType::new(K::to_data_type(), V::to_data_type(), true).into()
    }
}

/// The [`delta_kernel_derive::ToSchema`] macro uses this to convert a struct field's name + type
/// into a `StructField` definition. A blanket impl for `Option<T: ToDataType>` supports nullable
/// struct fields, which otherwise default to non-nullable.
#[internal_api]
pub(crate) trait GetStructField {
    fn get_struct_field(name: impl Into<String>) -> StructField;
    fn resource_field_bytes(name: &str) -> crate::DeltaResult<usize>;
}

// Normal types produce non-nullable fields
impl<T: ToDataType> GetStructField for T {
    fn resource_field_bytes(name: &str) -> crate::DeltaResult<usize> {add(add(name.len(),name.len())?,T::resource_data_type_bytes()?)}

    fn get_struct_field(name: impl Into<String>) -> StructField {
        StructField::not_null(name, T::to_data_type())
    }
}

// Option types produce nullable fields
impl<T: ToDataType> GetStructField for Option<T> {
    fn resource_field_bytes(name: &str) -> crate::DeltaResult<usize> {add(add(name.len(),name.len())?,T::resource_data_type_bytes()?)}

    fn get_struct_field(name: impl Into<String>) -> StructField {
        StructField::nullable(name, T::to_data_type())
    }
}

/// The [`delta_kernel_derive::ToSchema`] macro uses this trait to implement the
/// `allow_null_container_values` attribute. It is similar to [`ToDataType`], except the containers
/// it produces have nullable elements, e.g. [`MapType::value_contains_null`] is true.
pub(crate) trait ToNullableContainerType {
    fn to_nullable_container_type() -> DataType;
    fn nullable_container_type_bytes() -> crate::DeltaResult<usize>;
}

// Blanket impl for maps with nullable values
impl<K: ToDataType, V: ToDataType> ToNullableContainerType for HashMap<K, V> {
    fn nullable_container_type_bytes() -> crate::DeltaResult<usize> {add(add(K::resource_data_type_bytes()?,V::resource_data_type_bytes()?)?,3+std::mem::size_of::<MapType>())}

    fn to_nullable_container_type() -> DataType {
        MapType::new(K::to_data_type(), V::to_data_type(), true).into()
    }
}

// The [`delta_kernel_derive::ToSchema`] macro uses this to convert a struct field's name + type
// into a `StructField` definition for a container with nullable values, when the struct field was
// annotated with the `allow_null_container_values` attribute.
#[internal_api]
pub(crate) trait GetNullableContainerStructField {
    fn get_nullable_container_struct_field(name: impl Into<String>) -> StructField;
    fn nullable_container_field_bytes(name: &str) -> crate::DeltaResult<usize>;
}

// Blanket impl for all container types with nullable values
impl<T: ToNullableContainerType> GetNullableContainerStructField for T {
    fn nullable_container_field_bytes(name:&str)->crate::DeltaResult<usize>{add(add(name.len(),name.len())?,T::nullable_container_type_bytes()?)}

    fn get_nullable_container_struct_field(name: impl Into<String>) -> StructField {
        StructField::not_null(name, T::to_nullable_container_type())
    }
}

// Optional container types produce nullable fields with nullable values.
impl<T: ToNullableContainerType> GetNullableContainerStructField for Option<T> {
    fn nullable_container_field_bytes(name:&str)->crate::DeltaResult<usize>{add(add(name.len(),name.len())?,T::nullable_container_type_bytes()?)}

    fn get_nullable_container_struct_field(name: impl Into<String>) -> StructField {
        StructField::nullable(name, T::to_nullable_container_type())
    }
}

fn add(a:usize,b:usize)->crate::DeltaResult<usize>{a.checked_add(b).filter(|n|*n<=isize::MAX as usize).ok_or_else(||crate::resource::ResourceExhausted{kind:"native_derived_schema_layout",requested:usize::MAX,limit:isize::MAX as usize}.into())}
/// Source-native constructor layout used by the generated derive, before `to_schema` runs.
#[internal_api]
pub(crate) fn derived_struct_bytes(field_bytes:&[usize])->crate::DeltaResult<usize>{
    super::resource::derived_struct_bytes(field_bytes.len(),field_bytes.iter().copied().try_fold(0,add)?)
}
