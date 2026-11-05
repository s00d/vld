//! # vld-sea — SeaORM integration for `vld`
//!
//! Validate [`ActiveModel`](crate::sea_orm::ActiveModelTrait) fields **before**
//! `insert()` / `update()` hits the database.
//!
//! ## Approach
//!
//! 1. Define a `vld::schema!` that mirrors the entity columns you want validated.
//! 2. Call [`validate_active`] (extracts `Set`/`Unchanged` values from the
//!    ActiveModel into JSON and runs the schema).
//! 3. Or call [`validate_model`] on any `Serialize`-able struct (e.g. an input
//!    DTO, or SeaORM `Model`).
//! 4. Optionally hook into
//!    [`ActiveModelBehavior::before_save`](crate::sea_orm::ActiveModelBehavior::before_save)
//!    so validation runs automatically.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use sea_orm::*;
//! use vld_sea::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct UserInput {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! // Before insert:
//! let am = user::ActiveModel {
//!     name: Set("Alice".to_owned()),
//!     email: Set("alice@example.com".to_owned()),
//!     ..Default::default()
//! };
//! vld_sea::validate_active::<UserInput, _>(&am)?;
//! am.insert(&db).await?;
//! ```

#[cfg(all(feature = "sea-orm-1", feature = "sea-orm-2"))]
compile_error!("Enable exactly one SeaORM version feature: `sea-orm-1` or `sea-orm-2`.");
#[cfg(not(any(feature = "sea-orm-1", feature = "sea-orm-2")))]
compile_error!("Enable one SeaORM version feature: `sea-orm-1` or `sea-orm-2`.");

#[cfg(feature = "sea-orm-1")]
pub use sea_orm;
#[cfg(feature = "sea-orm-2")]
pub use sea_orm2 as sea_orm;

pub use vld;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

vld_integration_common::__vld_db_error!(
    /// Error returned by `vld-sea` operations.
    VldSeaError
);

impl From<VldSeaError> for crate::sea_orm::DbErr {
    fn from(e: VldSeaError) -> Self {
        crate::sea_orm::DbErr::Custom(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// ActiveModel → JSON conversion
// ---------------------------------------------------------------------------

/// Convert an [`ActiveModel`](crate::sea_orm::ActiveModelTrait) to a
/// [`serde_json::Value`] object.
///
/// Only fields with `Set` or `Unchanged` values are included.
/// Fields that are `NotSet` are omitted from the resulting object.
///
/// This function handles common SQL types (bool, integers, floats, strings,
/// chars). Feature-gated types (chrono, uuid, etc.) are mapped to
/// `serde_json::Value::Null`.
pub fn active_model_to_json<A>(model: &A) -> serde_json::Value
where
    A: crate::sea_orm::ActiveModelTrait,
{
    use crate::sea_orm::{ActiveValue, EntityTrait, IdenStatic, Iterable};

    let mut map = serde_json::Map::new();
    for col in <<A as crate::sea_orm::ActiveModelTrait>::Entity as EntityTrait>::Column::iter() {
        match model.get(col) {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => {
                map.insert(col.as_str().to_string(), sea_value_to_json(v));
            }
            ActiveValue::NotSet => {} // skip — not being set
        }
    }
    serde_json::Value::Object(map)
}

/// Convert a [`crate::sea_orm::Value`] to [`serde_json::Value`], matching what
/// `serde_json::to_value` produces for the corresponding Rust type (so the same
/// schema works for `validate_active` and `validate_model`):
///
/// - numbers / bools / strings / chars as-is, `Bytes` as an array of `u8`
/// - `with-json`: JSON as-is
/// - `with-chrono`: dates `YYYY-MM-DD`, times `HH:MM:SS[.f]`, naive datetimes
///   `YYYY-MM-DDTHH:MM:SS[.f]`, zoned datetimes RFC 3339
/// - `with-uuid`: hyphenated string; `with-rust_decimal` / `with-bigdecimal`: string
///
/// Variants whose vld-sea feature is not enabled (and `None`s) become `null`.
#[allow(unreachable_patterns)]
fn sea_value_to_json(v: crate::sea_orm::Value) -> serde_json::Value {
    use crate::sea_orm::sea_query::Value as SV;
    use serde_json::Value as J;

    fn float(n: f64) -> J {
        serde_json::Number::from_f64(n)
            .map(J::Number)
            .unwrap_or(J::Null)
    }

    match v {
        SV::Bool(Some(b)) => J::Bool(b),
        SV::TinyInt(Some(n)) => (n as i64).into(),
        SV::SmallInt(Some(n)) => (n as i64).into(),
        SV::Int(Some(n)) => (n as i64).into(),
        SV::BigInt(Some(n)) => n.into(),
        SV::TinyUnsigned(Some(n)) => (n as u64).into(),
        SV::SmallUnsigned(Some(n)) => (n as u64).into(),
        SV::Unsigned(Some(n)) => (n as u64).into(),
        SV::BigUnsigned(Some(n)) => n.into(),
        SV::Float(Some(n)) => float(n as f64),
        SV::Double(Some(n)) => float(n),
        SV::String(Some(s)) => J::String(s.to_string()),
        SV::Char(Some(c)) => J::String(c.to_string()),
        SV::Bytes(Some(b)) => J::Array(b.iter().map(|x| J::from(*x)).collect()),
        // sea-query 1 / sea-orm 2 — ActiveEnum columns
        #[cfg(feature = "sea-orm-2")]
        SV::Enum(opt) => match opt {
            crate::sea_orm::sea_query::OptionEnum::Some(e) => J::String(e.value.to_string()),
            crate::sea_orm::sea_query::OptionEnum::None(_) => J::Null,
        },
        #[cfg(feature = "with-json")]
        SV::Json(Some(j)) => (*j).clone(),
        #[cfg(feature = "with-chrono")]
        SV::ChronoDate(Some(d)) => J::String(d.to_string()),
        #[cfg(feature = "with-chrono")]
        SV::ChronoTime(Some(t)) => J::String(t.to_string()),
        #[cfg(feature = "with-chrono")]
        SV::ChronoDateTime(Some(dt)) => J::String(dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string()),
        #[cfg(feature = "with-chrono")]
        SV::ChronoDateTimeUtc(Some(dt)) => J::String(dt.to_rfc3339()),
        #[cfg(feature = "with-chrono")]
        SV::ChronoDateTimeLocal(Some(dt)) => J::String(dt.to_rfc3339()),
        #[cfg(feature = "with-chrono")]
        SV::ChronoDateTimeWithTimeZone(Some(dt)) => J::String(dt.to_rfc3339()),
        #[cfg(feature = "with-uuid")]
        SV::Uuid(Some(u)) => J::String(u.to_string()),
        #[cfg(feature = "with-rust_decimal")]
        SV::Decimal(Some(d)) => J::String(d.to_string()),
        #[cfg(feature = "with-bigdecimal")]
        SV::BigDecimal(Some(d)) => J::String(d.to_string()),
        // None variants and types without a vld-sea feature (time, arrays, ...)
        _ => J::Null,
    }
}

#[cfg(test)]
mod sea_value_tests {
    use super::sea_value_to_json;
    use crate::sea_orm::sea_query::Value as SV;
    use serde_json::json;

    #[test]
    #[allow(clippy::useless_conversion)] // Box<Vec<u8>> on sea-orm 1, Vec<u8> on 2
    fn bytes_map_to_u8_array() {
        let v = SV::Bytes(Some(vec![1u8, 2, 255].into()));
        assert_eq!(sea_value_to_json(v), json!([1, 2, 255]));
    }

    #[cfg(all(
        feature = "with-chrono",
        feature = "with-uuid",
        feature = "with-json",
        feature = "with-rust_decimal"
    ))]
    #[test]
    fn feature_types_map_like_serde() {
        use crate::sea_orm::prelude::{
            ChronoDate, ChronoDateTime, ChronoDateTimeUtc, Decimal, Uuid,
        };
        let date = ChronoDate::from_ymd_opt(2024, 2, 29).unwrap();
        let dt: ChronoDateTime = date.and_hms_opt(12, 30, 0).unwrap();
        let utc: ChronoDateTimeUtc = dt.and_utc();
        let id = Uuid::nil();
        let dec: Decimal = "12.50".parse().unwrap();

        assert_eq!(sea_value_to_json(date.into()), json!("2024-02-29"));
        assert_eq!(sea_value_to_json(dt.into()), json!("2024-02-29T12:30:00"));
        assert_eq!(
            sea_value_to_json(utc.into()),
            json!("2024-02-29T12:30:00+00:00")
        );
        assert_eq!(
            sea_value_to_json(id.into()),
            json!("00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(sea_value_to_json(dec.into()), json!("12.50"));
        assert_eq!(
            sea_value_to_json(json!({"a": [1, 2]}).into()),
            json!({"a": [1, 2]})
        );
    }
}

#[cfg(all(test, feature = "sea-orm-2"))]
mod sea_value_enum_tests {
    use super::sea_value_to_json;
    use crate::sea_orm::sea_query::{Enum, OptionEnum, Value as SV};

    #[test]
    fn enum_some_maps_to_string() {
        let v = SV::Enum(OptionEnum::Some(Box::new(Enum {
            type_name: "status".into(),
            value: "active".into(),
        })));
        assert_eq!(sea_value_to_json(v), serde_json::json!("active"));
    }

    #[test]
    fn enum_none_maps_to_null() {
        let v = SV::Enum(OptionEnum::None("status".into()));
        assert_eq!(sea_value_to_json(v), serde_json::Value::Null);
    }
}

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

/// Validate an [`ActiveModel`](crate::sea_orm::ActiveModelTrait) against schema `S`.
///
/// Extracts all `Set` and `Unchanged` fields into a JSON object and runs
/// `S::vld_parse_value()`.
///
/// Use this in [`ActiveModelBehavior::before_save`](crate::sea_orm::ActiveModelBehavior::before_save)
/// to validate before every insert/update.
///
/// ```rust,ignore
/// vld_sea::validate_active::<UserInput, _>(&active_model)?;
/// ```
pub fn validate_active<S, A>(model: &A) -> Result<S, VldSeaError>
where
    S: vld::schema::VldParse,
    A: crate::sea_orm::ActiveModelTrait,
{
    let json = active_model_to_json(model);
    S::vld_parse_value(&json).map_err(VldSeaError::Validation)
}

/// Validate a serializable value against schema `S`.
///
/// Works with SeaORM `Model`, input DTOs, or any `Serialize`-able struct.
///
/// ```rust,ignore
/// #[derive(serde::Serialize)]
/// struct NewUser { name: String, email: String }
///
/// let input = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
/// vld_sea::validate_model::<UserInput, _>(&input)?;
/// ```
pub fn validate_model<S, T>(value: &T) -> Result<S, VldSeaError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    __vld_validate::<S, T>(value)
}

/// Parse and validate raw JSON against schema `S`.
pub fn validate_json<S>(json: &serde_json::Value) -> Result<S, VldSeaError>
where
    S: vld::schema::VldParse,
{
    S::vld_parse_value(json).map_err(VldSeaError::Validation)
}

/// Helper for use inside
/// [`ActiveModelBehavior::before_save`](crate::sea_orm::ActiveModelBehavior::before_save).
///
/// Returns `Ok(())` on success or `Err(DbErr::Custom(...))` on failure,
/// so it can be used directly with `?` in the `before_save` method.
///
/// ```rust,ignore
/// #[async_trait::async_trait]
/// impl ActiveModelBehavior for ActiveModel {
///     async fn before_save<C: ConnectionTrait>(
///         self, _db: &C, _insert: bool,
///     ) -> Result<Self, DbErr> {
///         vld_sea::before_save::<UserInput, _>(&self)?;
///         Ok(self)
///     }
/// }
/// ```
pub fn before_save<S, A>(model: &A) -> Result<(), crate::sea_orm::DbErr>
where
    S: vld::schema::VldParse,
    A: crate::sea_orm::ActiveModelTrait,
{
    validate_active::<S, A>(model)
        .map(|_| ())
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Validated<S, T> — wrapper
// ---------------------------------------------------------------------------

vld_integration_common::__vld_db_validated!(VldSeaError);

// ---------------------------------------------------------------------------
// Macro: impl_vld_before_save!
// ---------------------------------------------------------------------------

/// Implements [`ActiveModelBehavior`](crate::sea_orm::ActiveModelBehavior) with
/// automatic vld validation in `before_save`.
///
/// ```rust,ignore
/// // In your entity module:
/// vld_sea::impl_vld_before_save!(ActiveModel, UserInput);
/// ```
///
/// This expands to an `ActiveModelBehavior` impl that calls
/// [`validate_active`] before every insert/update.
#[macro_export]
macro_rules! impl_vld_before_save {
    ($active_model:ty, $schema:ty) => {
        #[$crate::sea_orm::prelude::async_trait::async_trait]
        impl $crate::sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C: $crate::sea_orm::ConnectionTrait>(
                self,
                _db: &C,
                _insert: bool,
            ) -> Result<Self, $crate::sea_orm::DbErr> {
                $crate::before_save::<$schema, _>(&self)?;
                Ok(self)
            }
        }
    };
    ($active_model:ty, insert: $ins_schema:ty, update: $upd_schema:ty) => {
        #[$crate::sea_orm::prelude::async_trait::async_trait]
        impl $crate::sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C: $crate::sea_orm::ConnectionTrait>(
                self,
                _db: &C,
                insert: bool,
            ) -> Result<Self, $crate::sea_orm::DbErr> {
                if insert {
                    $crate::before_save::<$ins_schema, _>(&self)?;
                } else {
                    $crate::before_save::<$upd_schema, _>(&self)?;
                }
                Ok(self)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        active_model_to_json, before_save, validate_active, validate_json, validate_model,
        Validated, VldSeaError,
    };
    pub use vld::prelude::*;
}
