//! The SQLx integration, written once against `super::sqlx` and compiled by
//! `sqlx08.rs` / `sqlx09.rs`; what differs between the majors is in their
//! `compat` modules.

use super::compat::SqlxArgBuf;
use super::sqlx;

// ========================= Error / Validated / wrappers ======================

crate::db::__vld_db_error!(
    /// Error returned by this module's operations.
    VldSqlxError
);

/// A validation failure happens before a value is written: it becomes
/// [`sqlx::Error::Encode`] (not `Protocol`, which means a broken connection).
impl From<VldSqlxError> for sqlx::Error {
    fn from(e: VldSqlxError) -> Self {
        sqlx::Error::Encode(Box::new(e))
    }
}

crate::db::__vld_db_validated!(VldSqlxError);

crate::db::__vld_db_wrappers! {
    VldSqlxError;
    text: []
    int: []
    float: []
    bool: []
}

// ========================= Standalone helpers ================================

/// Validate a value against schema `S` before inserting (or updating).
pub fn validate_insert<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    __vld_validate::<S, T>(value).map(|_| ())
}

/// Validate a row loaded from the database against schema `S`
/// (same check as [`validate_insert`]).
pub fn validate_row<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

/// Validate a batch of rows against schema `S`.
///
/// Returns the index and error of the first invalid row.
pub fn validate_rows<S, T>(rows: &[T]) -> Result<(), (usize, VldSqlxError)>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    for (i, row) in rows.iter().enumerate() {
        validate_row::<S, T>(row).map_err(|e| (i, e))?;
    }
    Ok(())
}

// ========================= SQLx trait impls ==================================
//
// Encode/Decode delegate to the inner Rust type. Decoding does not re-validate
// (data from the DB is trusted; use `validate_row` to re-check).

macro_rules! sqlx_delegate {
    ($Name:ident, $inner:ty) => {
        impl<S, DB: sqlx::Database> sqlx::Type<DB> for $Name<S>
        where
            $inner: sqlx::Type<DB>,
        {
            fn type_info() -> DB::TypeInfo {
                <$inner as sqlx::Type<DB>>::type_info()
            }
            fn compatible(ty: &DB::TypeInfo) -> bool {
                <$inner as sqlx::Type<DB>>::compatible(ty)
            }
        }

        impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for $Name<S>
        where
            $inner: sqlx::Encode<'q, DB>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut SqlxArgBuf<'q, DB>,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                self.value.encode_by_ref(buf)
            }
        }

        impl<'r, S, DB: sqlx::Database> sqlx::Decode<'r, DB> for $Name<S>
        where
            $inner: sqlx::Decode<'r, DB>,
        {
            fn decode(
                value: <DB as sqlx::Database>::ValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                Ok(Self::new_unchecked(
                    <$inner as sqlx::Decode<'r, DB>>::decode(value)?,
                ))
            }
        }
    };
}

sqlx_delegate!(VldText, String);
sqlx_delegate!(VldBool, bool);

// VldFloat: accepts DOUBLE and REAL / FLOAT4 columns, encodes as f64. A REAL
// is widened through its decimal form, so `0.1f32` reads back as `0.1`.
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldFloat<S>
where
    f64: sqlx::Type<DB>,
    f32: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <f64 as sqlx::Type<DB>>::type_info()
    }
    fn compatible(ty: &DB::TypeInfo) -> bool {
        <f64 as sqlx::Type<DB>>::compatible(ty) || <f32 as sqlx::Type<DB>>::compatible(ty)
    }
}

impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldFloat<S>
where
    f64: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut SqlxArgBuf<'q, DB>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

impl<'r, S, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldFloat<S>
where
    f64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    f32: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        use sqlx::ValueRef as _;
        let ty = value.type_info().into_owned();
        let v = if !<f64 as sqlx::Type<DB>>::compatible(&ty)
            && <f32 as sqlx::Type<DB>>::compatible(&ty)
        {
            let f = <f32 as sqlx::Decode<'r, DB>>::decode(value)?;
            f.to_string().parse().unwrap_or(f64::from(f))
        } else {
            <f64 as sqlx::Decode<'r, DB>>::decode(value)?
        };
        Ok(Self::new_unchecked(v))
    }
}

// VldInt: accepts BIGINT, INTEGER and SMALLINT columns (Postgres INT8/INT4/INT2
// are distinct types), encodes as i64.
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldInt<S>
where
    i64: sqlx::Type<DB>,
    i32: sqlx::Type<DB>,
    i16: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <i64 as sqlx::Type<DB>>::type_info()
    }
    fn compatible(ty: &DB::TypeInfo) -> bool {
        <i64 as sqlx::Type<DB>>::compatible(ty)
            || <i32 as sqlx::Type<DB>>::compatible(ty)
            || <i16 as sqlx::Type<DB>>::compatible(ty)
    }
}

impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldInt<S>
where
    i64: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut SqlxArgBuf<'q, DB>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

impl<'r, S, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldInt<S>
where
    i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    i32: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    i16: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        use sqlx::ValueRef as _;
        let ty = value.type_info().into_owned();
        let v = if <i64 as sqlx::Type<DB>>::compatible(&ty) {
            <i64 as sqlx::Decode<'r, DB>>::decode(value)?
        } else if <i32 as sqlx::Type<DB>>::compatible(&ty) {
            <i32 as sqlx::Decode<'r, DB>>::decode(value)?.into()
        } else if <i16 as sqlx::Type<DB>>::compatible(&ty) {
            <i16 as sqlx::Decode<'r, DB>>::decode(value)?.into()
        } else {
            <i64 as sqlx::Decode<'r, DB>>::decode(value)?
        };
        Ok(Self::new_unchecked(v))
    }
}

// ========================= Prelude ===========================================

/// Common imports: the sqlx wrappers plus `vld::prelude`.
pub mod prelude {
    pub use super::{
        Validated, VldBool, VldFloat, VldInt, VldSqlxError, VldText, validate_insert, validate_row,
        validate_rows,
    };
    pub use vld::prelude::*;
}
