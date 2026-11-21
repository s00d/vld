//! # `vld_data::postgres_types` — tokio-postgres / postgres for `vld`
//!
//! Column types that are validated against a `vld` schema when they are
//! built and again when they are read back, implementing
//! [`postgres_types::ToSql`] / [`postgres_types::FromSql`]. They work with
//! `tokio-postgres` and the synchronous `postgres` crate (both use
//! `postgres-types` 0.2):
//!
//! - [`Validated<S, T>`] — a `JSON` / `JSONB` column: `T` is sent as JSON;
//!   on read the JSON is validated against `S`, then deserialized into `T`.
//! - [`VldText`], [`VldInt`] (`INT8`), [`VldFloat`] (`FLOAT8`), [`VldBool`] —
//!   scalar columns checked as `{"value": v}` against `S` (the schema needs
//!   a `value` field).
//!
//! A value that fails validation on read is a `FromSql` error, which
//! tokio-postgres reports from `Row::try_get` (and `Row::get` panics with it).
//!
//! Feature `tokio-postgres`.
//!
//! ```rust
//! use vld_data::postgres_types::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct ProfileSchema {
//!         pub nickname: String => vld::string().min(2),
//!     }
//! }
//!
//! #[derive(Debug, serde::Serialize, serde::Deserialize)]
//! struct Profile { nickname: String }
//!
//! // client.execute("INSERT INTO users (profile) VALUES ($1)", &[&profile])
//! let profile = Validated::<ProfileSchema, _>::new(Profile { nickname: "ada".into() }).unwrap();
//! assert!(Validated::<ProfileSchema, _>::new(Profile { nickname: "a".into() }).is_err());
//! // let profile: Validated<ProfileSchema, Profile> = row.try_get("profile")?;
//! # let _ = profile;
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/tokio-postgres>

use bytes::BytesMut;
use postgres_types::{FromSql, IsNull, Json, ToSql, Type, to_sql_checked};
use std::error::Error;

pub use vld;

/// What `ToSql` / `FromSql` return as an error.
type BoxError = Box<dyn Error + Sync + Send>;

crate::db::__vld_db_error!(
    /// Error of `vld_data::postgres_types`: a value failed its schema or
    /// could not be serialized to JSON. Returned (boxed) from `FromSql`.
    VldPgError
);

crate::db::__vld_db_validated!(VldPgError);

crate::db::__vld_db_wrappers!(VldPgError; text: [] int: [] float: [] bool: []);

impl<S, T> ToSql for Validated<S, T>
where
    T: serde::Serialize + std::fmt::Debug,
{
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        Json(&self.inner).to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <Json<serde_json::Value> as ToSql>::accepts(ty)
    }

    to_sql_checked!();
}

impl<'a, S, T> FromSql<'a> for Validated<S, T>
where
    S: vld::schema::VldParse,
    T: serde::de::DeserializeOwned,
{
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
        let json = serde_json::Value::from_sql(ty, raw)?;
        S::vld_parse_value(&json).map_err(VldPgError::Validation)?;
        Ok(Self {
            inner: serde_json::from_value(json)?,
            _schema: std::marker::PhantomData,
        })
    }

    fn accepts(ty: &Type) -> bool {
        <serde_json::Value as FromSql>::accepts(ty)
    }
}

/// `ToSql` / `FromSql` of a scalar wrapper through its inner type; `FromSql`
/// validates the value read.
macro_rules! pg_scalar {
    ($Name:ident, $inner:ty) => {
        impl<S> ToSql for $Name<S> {
            fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
                <$inner as ToSql>::to_sql(&self.value, ty, out)
            }

            fn accepts(ty: &Type) -> bool {
                <$inner as ToSql>::accepts(ty)
            }

            to_sql_checked!();
        }

        impl<'a, S: vld::schema::VldParse> FromSql<'a> for $Name<S> {
            fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
                Ok(Self::new(<$inner as FromSql>::from_sql(ty, raw)?)?)
            }

            fn accepts(ty: &Type) -> bool {
                <$inner as FromSql>::accepts(ty)
            }
        }
    };
}

pg_scalar!(VldText, String);
pg_scalar!(VldInt, i64);
pg_scalar!(VldFloat, f64);
pg_scalar!(VldBool, bool);

/// Common imports: `Validated`, the scalar wrappers, `VldPgError` and
/// `vld::prelude`.
pub mod prelude {
    pub use super::{Validated, VldBool, VldFloat, VldInt, VldPgError, VldText};
    pub use vld::prelude::*;
}
