//! # vld-diesel — Diesel integration for the `vld` validation library
//!
//! Validate data **before** inserting into the database, and use strongly-typed
//! validated column types.
//!
//! ## Quick Start
//!
//! ```ignore
//! use vld_diesel::prelude::*;
//!
//! // 1. Define a vld schema for your insertable struct
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NewUserSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! // 2. Wrap your Diesel Insertable with Validated
//! let new_user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! let validated = Validated::<NewUserSchema, _>::new(new_user)?;
//!
//! // 3. Insert — the inner value is guaranteed to be valid
//! diesel::insert_into(users::table)
//!     .values(validated.inner())
//!     .execute(&mut conn)?;
//! ```
//!
//! ## Validated column types
//!
//! Use [`VldText`] for columns that must always satisfy a validation schema:
//!
//! ```ignore
//! use vld_diesel::VldText;
//!
//! let email = VldText::<EmailSchema>::new("user@example.com")?;
//! ```

pub use vld;

// ---------------------------------------------------------------------------
// Error / Validated / column wrappers (shared via vld-integration-common)
// ---------------------------------------------------------------------------

vld_integration_common::__vld_db_error!(
    /// Error returned by `vld-diesel` operations.
    VldDieselError
);

vld_integration_common::__vld_db_validated!(VldDieselError);

// The wrappers derive `AsExpression`/`FromSqlRow`, so they work in
// `#[derive(Insertable, Queryable)]` structs, `.filter(col.eq(wrapper))`, etc.
vld_integration_common::__vld_db_wrappers! {
    VldDieselError;
    text: [
        #[derive(diesel::expression::AsExpression, diesel::deserialize::FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::Text)]
    ]
    int: [
        #[derive(diesel::expression::AsExpression, diesel::deserialize::FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::BigInt)]
    ]
    float: [
        #[derive(diesel::expression::AsExpression, diesel::deserialize::FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::Double)]
    ]
    bool: [
        #[derive(diesel::expression::AsExpression, diesel::deserialize::FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::Bool)]
    ]
}

// ---------------------------------------------------------------------------
// validate_insert / validate_row — standalone helpers
// ---------------------------------------------------------------------------

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
/// vld_diesel::validate_insert::<ItemSchema, _>(&item).unwrap();
///
/// let v = vld_diesel::Validated::<ItemSchema, _>::new(item).unwrap();
/// assert_eq!(v.inner().qty, 5);
/// ```
pub fn validate_insert<S, T>(value: &T) -> Result<(), VldDieselError>
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
pub fn validate_update<S, T>(value: &T) -> Result<(), VldDieselError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

/// Validate a row loaded from the database against schema `S`
/// (same check as [`validate_insert`]).
pub fn validate_row<S, T>(value: &T) -> Result<(), VldDieselError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

// ---------------------------------------------------------------------------
// Diesel ToSql / FromSql — delegate to the inner Rust type.
// FromSql does not re-validate (trusted DB data; use `validate_row`).
// ---------------------------------------------------------------------------

macro_rules! diesel_delegate {
    ($Name:ident, $to:ty, $inner:ty, $st:ty) => {
        impl<S, DB> diesel::serialize::ToSql<$st, DB> for $Name<S>
        where
            DB: diesel::backend::Backend,
            $to: diesel::serialize::ToSql<$st, DB>,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut diesel::serialize::Output<'b, '_, DB>,
            ) -> diesel::serialize::Result {
                <$to as diesel::serialize::ToSql<$st, DB>>::to_sql(&self.value, out)
            }
        }

        impl<S, DB> diesel::deserialize::FromSql<$st, DB> for $Name<S>
        where
            DB: diesel::backend::Backend,
            $inner: diesel::deserialize::FromSql<$st, DB>,
        {
            fn from_sql(
                bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
            ) -> diesel::deserialize::Result<Self> {
                Ok(Self::new_unchecked(
                    <$inner as diesel::deserialize::FromSql<$st, DB>>::from_sql(bytes)?,
                ))
            }
        }
    };
}

diesel_delegate!(VldText, str, String, diesel::sql_types::Text);
diesel_delegate!(VldInt, i64, i64, diesel::sql_types::BigInt);
diesel_delegate!(VldFloat, f64, f64, diesel::sql_types::Double);
diesel_delegate!(VldBool, bool, bool, diesel::sql_types::Bool);

// FromSql for Integer (i32) — useful when the DB column is INTEGER.
impl<S, DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for VldInt<S>
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(
        bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let v =
            <i32 as diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>>::from_sql(bytes)?;
        Ok(Self::new_unchecked(v))
    }
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

pub mod prelude {
    #[allow(deprecated)]
    pub use crate::{
        validate_insert, validate_row, validate_update, Validated, VldBool, VldDieselError,
        VldFloat, VldInt, VldText,
    };
    pub use vld::prelude::*;
}
