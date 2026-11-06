//! # vld-diesel — Diesel integration for `vld`
//!
//! Validate values against a [`vld`] schema before Diesel writes them
//! ([`validate_insert`], [`Validated`]) or after loading them
//! ([`validate_row`]), and use validated column types ([`VldText`],
//! [`VldInt`], [`VldFloat`], [`VldBool`]). The wrappers derive
//! `AsExpression` / `FromSqlRow`, so they work in `#[derive(Insertable,
//! Queryable)]` structs and in `.filter(col.eq(wrapper))`.
//!
//! Features: `sqlite` (default), `postgres`, `mysql`.
//!
//! ```
//! use diesel::prelude::*;
//! use vld_diesel::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NewUserSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! diesel::table! {
//!     users (id) {
//!         id -> Integer,
//!         name -> Text,
//!         email -> Text,
//!     }
//! }
//!
//! #[derive(Insertable, serde::Serialize)]
//! #[diesel(table_name = users)]
//! struct NewUser { name: String, email: String }
//!
//! let mut conn = SqliteConnection::establish(":memory:").unwrap();
//! diesel::sql_query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL)")
//!     .execute(&mut conn)
//!     .unwrap();
//!
//! let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! let validated = Validated::<NewUserSchema, _>::new(user).unwrap();
//! diesel::insert_into(users::table)
//!     .values(validated.inner())
//!     .execute(&mut conn)
//!     .unwrap();
//! ```
//!
//! Column wrappers validate `{"value": ...}`, so their schema needs a `value`
//! field. `FromSql` does not re-validate (data from the database is trusted);
//! call [`validate_row`] to re-check loaded rows.
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/diesel>

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
        #[diesel(sql_type = diesel::sql_types::Integer)]
    ]
    float: [
        #[derive(diesel::expression::AsExpression, diesel::deserialize::FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::Double)]
        #[diesel(sql_type = diesel::sql_types::Float)]
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

// FromSql for Integer (i32) — for INTEGER columns (`VldInt` is also
// `AsExpression<Integer>`; writing checks the value fits an i32).
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

// FromSql for Float (f32 / REAL): widened through its shortest decimal form,
// so `0.1f32` reads back as `0.1`, not `0.10000000149011612`.
impl<S, DB> diesel::deserialize::FromSql<diesel::sql_types::Float, DB> for VldFloat<S>
where
    DB: diesel::backend::Backend,
    f32: diesel::deserialize::FromSql<diesel::sql_types::Float, DB>,
{
    fn from_sql(
        bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let v =
            <f32 as diesel::deserialize::FromSql<diesel::sql_types::Float, DB>>::from_sql(bytes)?;
        Ok(Self::new_unchecked(widen_f32(v)))
    }
}

/// `f32` → `f64` keeping the decimal value the `f32` prints as.
fn widen_f32(v: f32) -> f64 {
    v.to_string().parse().unwrap_or(f64::from(v))
}

/// `i64` → `i32` for an INTEGER column, failing instead of truncating.
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
fn narrow_int(v: i64) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    i32::try_from(v).map_err(|_| format!("{v} does not fit an INTEGER (i32) column").into())
}

/// `f64` → `f32` for a REAL / FLOAT column, failing when out of `f32` range.
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
fn narrow_float(v: f64) -> Result<f32, Box<dyn std::error::Error + Send + Sync>> {
    let f = v as f32;
    if v.is_finite() && !f.is_finite() {
        return Err(format!("{v} does not fit a FLOAT (f32) column").into());
    }
    Ok(f)
}

// ToSql for Integer / Float: a checked narrowing written per backend (diesel's
// generic `ToSql` borrows the value for the whole query, which a narrowed
// temporary cannot provide).
#[cfg(feature = "sqlite")]
mod sqlite_narrow {
    use super::*;
    use diesel::serialize::{IsNull, Output, ToSql};
    use diesel::sqlite::Sqlite;

    impl<S> ToSql<diesel::sql_types::Integer, Sqlite> for VldInt<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
            out.set_value(narrow_int(self.value)?);
            Ok(IsNull::No)
        }
    }

    impl<S> ToSql<diesel::sql_types::Float, Sqlite> for VldFloat<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
            // SQLite stores REAL as a double: keep the f64.
            narrow_float(self.value)?;
            out.set_value(self.value);
            Ok(IsNull::No)
        }
    }
}

#[cfg(feature = "postgres")]
mod pg_narrow {
    use super::*;
    use diesel::pg::Pg;
    use diesel::serialize::{IsNull, Output, ToSql};
    use std::io::Write;

    impl<S> ToSql<diesel::sql_types::Integer, Pg> for VldInt<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
            out.write_all(&narrow_int(self.value)?.to_be_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl<S> ToSql<diesel::sql_types::Float, Pg> for VldFloat<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
            out.write_all(&narrow_float(self.value)?.to_be_bytes())?;
            Ok(IsNull::No)
        }
    }
}

#[cfg(feature = "mysql")]
mod mysql_narrow {
    use super::*;
    use diesel::mysql::Mysql;
    use diesel::serialize::{IsNull, Output, ToSql};
    use std::io::Write;

    impl<S> ToSql<diesel::sql_types::Integer, Mysql> for VldInt<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Mysql>) -> diesel::serialize::Result {
            out.write_all(&narrow_int(self.value)?.to_ne_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl<S> ToSql<diesel::sql_types::Float, Mysql> for VldFloat<S> {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Mysql>) -> diesel::serialize::Result {
            out.write_all(&narrow_float(self.value)?.to_ne_bytes())?;
            Ok(IsNull::No)
        }
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
