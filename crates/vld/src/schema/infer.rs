//! [`VldType`]: the default schema of a Rust type.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;

use crate::collections::{ZArray, ZOrderedRecord, ZRecord, ZSet};
use crate::modifiers::ZOptional;
use crate::primitives::{ZBoolean, ZChar, ZF32, ZInteger, ZJsonValue, ZNumber, ZString};
use crate::schema::{Schema, VldSchema};

/// The schema a type gets when a `#[derive(Validate)]` field has no
/// `#[vld(...)]`: `String` → `vld::string()`, `u16` → `vld::u16()`,
/// `Option<T>` → `T`'s schema `.optional()`, `Vec<T>` → `vld::array(...)`,
/// `HashMap<String, T>` → `vld::record(...)`, a derived type → `vld::nested!`.
///
/// Derived non-generic structs and enums implement it; implement it for
/// your own types to make them inferable.
///
/// ```
/// use vld::schema::VldType;
/// use vld::prelude::*;
///
/// let tags = <Vec<String>>::vld_schema();
/// assert_eq!(tags.parse(r#"["a"]"#).unwrap(), ["a"]);
/// assert!(<u8>::vld_schema().parse("300").is_err());
/// ```
pub trait VldType: Sized {
    /// The schema type.
    type Schema: Schema<Output = Self>;
    /// A fresh schema with no extra checks.
    fn vld_schema() -> Self::Schema;
}

macro_rules! leaf {
    ($($ty:ty => $schema:ty = $make:expr;)*) => {$(
        impl VldType for $ty {
            type Schema = $schema;
            fn vld_schema() -> $schema {
                $make
            }
        }
    )*};
}

leaf! {
    String => ZString = crate::string();
    bool => ZBoolean = crate::boolean();
    char => ZChar = crate::char();
    f64 => ZNumber = crate::number();
    f32 => ZF32 = crate::f32();
    serde_json::Value => ZJsonValue = crate::json_value();
}

macro_rules! ints {
    ($($ty:ident)*) => {$(
        leaf! { $ty => ZInteger<$ty> = crate::$ty(); }
    )*};
}

ints!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

impl<T: VldType> VldType for Option<T> {
    type Schema = ZOptional<T::Schema>;
    fn vld_schema() -> Self::Schema {
        T::vld_schema().optional()
    }
}

impl<T: VldType> VldType for Vec<T> {
    type Schema = ZArray<T::Schema>;
    fn vld_schema() -> Self::Schema {
        crate::array(T::vld_schema())
    }
}

impl<T: VldType + Eq + Hash> VldType for HashSet<T> {
    type Schema = ZSet<T::Schema>;
    fn vld_schema() -> Self::Schema {
        crate::set(T::vld_schema())
    }
}

impl<T: VldType> VldType for HashMap<String, T> {
    type Schema = ZRecord<T::Schema>;
    fn vld_schema() -> Self::Schema {
        crate::record(T::vld_schema())
    }
}

impl<T: VldType> VldType for BTreeMap<String, T> {
    type Schema = ZOrderedRecord<T::Schema>;
    fn vld_schema() -> Self::Schema {
        crate::record(T::vld_schema()).ordered()
    }
}
