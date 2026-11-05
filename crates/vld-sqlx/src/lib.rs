//! # vld-sqlx — SQLx integration for the `vld` validation library
//!
//! Validate data **before** inserting into the database, and use strongly-typed
//! validated column types with SQLx.
//!
//! ## Quick Start
//!
//! ```ignore
//! use vld_sqlx::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NewUserSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct NewUser { name: String, email: String }
//!
//! let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! validate_insert::<NewUserSchema, _>(&user)?;
//! // now safe to insert via sqlx::query!(...)
//! ```

#[cfg(all(feature = "sqlx-0_8", feature = "sqlx-0_9"))]
compile_error!("Enable exactly one SQLx version feature: `sqlx-0_8` or `sqlx-0_9`.");
#[cfg(not(any(feature = "sqlx-0_8", feature = "sqlx-0_9")))]
compile_error!("Enable one SQLx version feature: `sqlx-0_8` or `sqlx-0_9`.");

#[cfg(feature = "sqlx-0_8")]
use sqlx08 as sqlx;
#[cfg(feature = "sqlx-0_9")]
use sqlx09 as sqlx;

pub use vld;

#[cfg(feature = "sqlx-0_8")]
type SqlxArgBuf<'q, DB> = <DB as sqlx::Database>::ArgumentBuffer<'q>;
#[cfg(feature = "sqlx-0_9")]
type SqlxArgBuf<'q, DB> = <DB as sqlx::Database>::ArgumentBuffer;

// ========================= Error / Validated / wrappers ======================

vld_integration_common::__vld_db_error!(
    /// Error returned by `vld-sqlx` operations.
    VldSqlxError
);

impl From<VldSqlxError> for sqlx::Error {
    fn from(e: VldSqlxError) -> Self {
        sqlx::Error::Protocol(e.to_string())
    }
}

vld_integration_common::__vld_db_validated!(VldSqlxError);

vld_integration_common::__vld_db_wrappers! {
    VldSqlxError;
    text: []
    int: []
    float: []
    bool: []
}

// ========================= Standalone helpers ================================

/// Validate a value against schema `S` before inserting (or updating).
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct ItemSchema {
///         pub name: String => vld::string().min(1),
///         pub qty: i64 => vld::number().int().min(0),
///     }
/// }
///
/// #[derive(serde::Serialize)]
/// struct NewItem { name: String, qty: i64 }
///
/// let item = NewItem { name: "Widget".into(), qty: 5 };
/// vld_sqlx::validate_insert::<ItemSchema, _>(&item).unwrap();
///
/// let v = vld_sqlx::Validated::<ItemSchema, _>::new(item).unwrap();
/// assert_eq!(v.inner().qty, 5);
/// ```
pub fn validate_insert<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    __vld_validate::<S, T>(value).map(|_| ())
}

/// Same as [`validate_insert`].
#[deprecated(
    since = "0.5.0",
    note = "identical to `validate_insert`; use it instead"
)]
pub fn validate_update<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
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
sqlx_delegate!(VldFloat, f64);
sqlx_delegate!(VldBool, bool);

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

#[cfg(all(test, feature = "postgres"))]
mod pg_tests {
    use super::*;
    use sqlx::postgres::Postgres;
    use sqlx::Type;

    struct AnySchema;

    #[test]
    fn vld_int_compatible_with_int4_and_int2() {
        for ty in [
            <i16 as Type<Postgres>>::type_info(),
            <i32 as Type<Postgres>>::type_info(),
            <i64 as Type<Postgres>>::type_info(),
        ] {
            assert!(<VldInt<AnySchema> as Type<Postgres>>::compatible(&ty));
        }
    }
}

// ========================= Prelude ===========================================

pub mod prelude {
    #[allow(deprecated)]
    pub use crate::{
        validate_insert, validate_row, validate_rows, validate_update, Validated, VldBool,
        VldFloat, VldInt, VldSqlxError, VldText,
    };
    pub use vld::prelude::*;
}
