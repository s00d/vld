//! # vld-sqlx — SQLx integration for `vld`
//!
//! Validate values against a [`vld`] schema before writing them with SQLx
//! ([`validate_insert`], [`Validated`]) or after loading them
//! ([`validate_row`], [`validate_rows`]), and bind validated column wrappers
//! ([`VldText`], [`VldInt`], [`VldFloat`], [`VldBool`]) that implement
//! `sqlx::Type`, `Encode` and `Decode`.
//!
//! Features: `sqlx-0_8` (default) or `sqlx-0_9` — exactly one; `sqlite`
//! (default), `postgres`, `mysql`.
//!
//! ```
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
//! // Column schemas validate `{"value": ...}`.
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct EmailField {
//!         pub value: String => vld::string().email(),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct NewUser { name: String, email: String }
//!
//! let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! validate_insert::<NewUserSchema, _>(&user).unwrap();
//! // sqlx::query("INSERT ...").bind(&user.name).bind(&user.email).execute(&pool).await?;
//!
//! let email = VldText::<EmailField>::new("carol@example.com").unwrap(); // bind(&email)
//! assert!(VldText::<EmailField>::new("bad").is_err());
//! # let _ = email;
//! ```
//!
//! Decoding does not re-validate (data from the database is trusted); call
//! [`validate_row`] to re-check loaded rows.
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/sqlx>

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

/// A validation failure happens before a value is written: it becomes
/// [`sqlx::Error::Encode`] (not `Protocol`, which means a broken connection).
impl From<VldSqlxError> for sqlx::Error {
    fn from(e: VldSqlxError) -> Self {
        sqlx::Error::Encode(Box::new(e))
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

#[cfg(all(test, feature = "postgres"))]
mod pg_tests {
    use super::*;
    use sqlx::Type;
    use sqlx::postgres::Postgres;

    struct AnySchema;

    #[test]
    fn vld_float_compatible_with_float4() {
        for ty in [
            <f32 as Type<Postgres>>::type_info(),
            <f64 as Type<Postgres>>::type_info(),
        ] {
            assert!(<VldFloat<AnySchema> as Type<Postgres>>::compatible(&ty));
        }
    }

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
        Validated, VldBool, VldFloat, VldInt, VldSqlxError, VldText, validate_insert, validate_row,
        validate_rows, validate_update,
    };
    pub use vld::prelude::*;
}
