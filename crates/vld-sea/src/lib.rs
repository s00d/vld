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

use std::fmt;
use std::ops::Deref;

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

/// Error returned by `vld-sea` operations.
#[derive(Debug, Clone)]
pub enum VldSeaError {
    /// Schema validation failed.
    Validation(vld::error::VldError),
    /// Failed to serialize the value to JSON for validation.
    Serialization(String),
}

impl fmt::Display for VldSeaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldSeaError::Validation(e) => write!(f, "Validation error: {}", e),
            VldSeaError::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for VldSeaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldSeaError::Validation(e) => Some(e),
            VldSeaError::Serialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldSeaError {
    fn from(e: vld::error::VldError) -> Self {
        VldSeaError::Validation(e)
    }
}

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

/// Convert a [`crate::sea_orm::Value`] to [`serde_json::Value`].
///
/// Handles the always-available variants. Feature-gated variants
/// (chrono, uuid, json, etc.) fall through to `Null`.
fn sea_value_to_json(v: crate::sea_orm::Value) -> serde_json::Value {
    use crate::sea_orm::sea_query::Value as SV;

    match v {
        SV::Bool(Some(b)) => serde_json::Value::Bool(b),
        SV::TinyInt(Some(n)) => serde_json::Value::Number((n as i64).into()),
        SV::SmallInt(Some(n)) => serde_json::Value::Number((n as i64).into()),
        SV::Int(Some(n)) => serde_json::Value::Number((n as i64).into()),
        SV::BigInt(Some(n)) => serde_json::Value::Number(n.into()),
        SV::TinyUnsigned(Some(n)) => serde_json::Value::Number((n as u64).into()),
        SV::SmallUnsigned(Some(n)) => serde_json::Value::Number((n as u64).into()),
        SV::Unsigned(Some(n)) => serde_json::Value::Number((n as u64).into()),
        SV::BigUnsigned(Some(n)) => serde_json::Value::Number(n.into()),
        SV::Float(Some(n)) => serde_json::Number::from_f64(n as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        SV::Double(Some(n)) => serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        SV::String(Some(s)) => serde_json::Value::String(s.to_string()),
        SV::Char(Some(c)) => serde_json::Value::String(c.to_string()),
        // sea-query 1 / sea-orm 2 — ActiveEnum columns
        #[cfg(feature = "sea-orm-2")]
        SV::Enum(opt) => match opt {
            crate::sea_orm::sea_query::OptionEnum::Some(e) => {
                serde_json::Value::String(e.value.to_string())
            }
            crate::sea_orm::sea_query::OptionEnum::None(_) => serde_json::Value::Null,
        },
        // Bytes, Chrono, Uuid, Json, etc. — or any None variant
        _ => serde_json::Value::Null,
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
    let json =
        serde_json::to_value(value).map_err(|e| VldSeaError::Serialization(e.to_string()))?;
    S::vld_parse_value(&json).map_err(VldSeaError::Validation)
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
    let json = active_model_to_json(model);
    S::vld_parse_value(&json)
        .map(|_| ())
        .map_err(|e| crate::sea_orm::DbErr::Custom(format!("Validation error: {}", e)))
}

// ---------------------------------------------------------------------------
// Validated<S, T> — wrapper
// ---------------------------------------------------------------------------

/// A wrapper that proves its inner value has been validated against schema `S`.
///
/// `T` must implement [`serde::Serialize`] so it can be converted to JSON
/// for validation.
///
/// ```rust,ignore
/// let input = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
/// let validated = vld_sea::Validated::<UserInput, _>::new(input)?;
/// // validated.inner() is guaranteed valid
/// ```
pub struct Validated<S, T> {
    inner: T,
    _schema: std::marker::PhantomData<S>,
}

impl<S, T> Validated<S, T>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    /// Validate `value` against schema `S` and wrap it on success.
    pub fn new(value: T) -> Result<Self, VldSeaError> {
        let json =
            serde_json::to_value(&value).map_err(|e| VldSeaError::Serialization(e.to_string()))?;
        S::vld_parse_value(&json).map_err(VldSeaError::Validation)?;
        Ok(Self {
            inner: value,
            _schema: std::marker::PhantomData,
        })
    }

    /// Get a reference to the validated inner value.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<S, T> Deref for Validated<S, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<S, T: fmt::Debug> fmt::Debug for Validated<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Validated")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, T: Clone> Clone for Validated<S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _schema: std::marker::PhantomData,
        }
    }
}

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
