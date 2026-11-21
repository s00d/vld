//! The SeaORM integration, written once against `super::sea_orm` and
//! compiled by `sea1.rs` / `sea2.rs`; what differs between the majors is in
//! their `compat` modules.

use super::sea_orm;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

crate::db::__vld_db_error!(
    /// Error returned by this module's operations.
    VldSeaError
);

impl From<VldSeaError> for sea_orm::DbErr {
    fn from(e: VldSeaError) -> Self {
        sea_orm::DbErr::Custom(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// ActiveModel → JSON conversion
// ---------------------------------------------------------------------------

/// Convert an [`ActiveModel`](sea_orm::ActiveModelTrait) to a
/// [`serde_json::Value`] object.
///
/// Only fields with `Set` or `Unchanged` values are included.
/// Fields that are `NotSet` are omitted from the resulting object.
///
/// Common SQL types (bool, integers, floats, strings, chars, bytes) are always
/// mapped; JSON, chrono, time, uuid, decimals and Postgres arrays need the
/// matching vld-data feature (`with-json`, `with-chrono`, `with-time`,
/// `with-uuid`, `with-rust_decimal`, `with-bigdecimal`, `postgres-array`) and
/// are `null` without it.
pub fn active_model_to_json<A>(model: &A) -> serde_json::Value
where
    A: sea_orm::ActiveModelTrait,
{
    use sea_orm::{ActiveValue, EntityTrait, IdenStatic, Iterable};

    let mut map = serde_json::Map::new();
    for col in <<A as sea_orm::ActiveModelTrait>::Entity as EntityTrait>::Column::iter() {
        match model.get(col) {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => {
                map.insert(col.as_str().to_string(), sea_value_to_json(v));
            }
            ActiveValue::NotSet => {} // skip — not being set
        }
    }
    serde_json::Value::Object(map)
}

/// Convert a [`sea_orm::Value`] to [`serde_json::Value`], matching what
/// `serde_json::to_value` produces for the corresponding Rust type (so the same
/// schema works for `validate_active` and `validate_model`):
///
/// - numbers / bools / strings / chars as-is, `Bytes` as an array of `u8`
/// - `with-json`: JSON as-is
/// - `with-chrono`: dates `YYYY-MM-DD`, times `HH:MM:SS[.f]`, naive datetimes
///   `YYYY-MM-DDTHH:MM:SS[.f]`, zoned datetimes RFC 3339
/// - `with-uuid`: hyphenated string; `with-rust_decimal` / `with-bigdecimal`: string
/// - `with-time`: the same ISO shapes as chrono (`±HH:MM` offsets)
/// - `postgres-array`: a JSON array of the mapped items
/// - `Float` (`f32`) through its decimal form (`0.1f32` → `0.1`)
/// - sea-orm 2 native enums (`Value::Enum`): the variant's string value
///
/// Variants whose vld-data feature is not enabled (and `None`s) become `null`.
#[allow(unreachable_patterns)]
fn sea_value_to_json(v: sea_orm::Value) -> serde_json::Value {
    use sea_orm::sea_query::Value as SV;
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
        // Through the decimal form: `0.1f32` maps to 0.1, not 0.10000000149.
        SV::Float(Some(n)) => float(n.to_string().parse().unwrap_or(f64::from(n))),
        SV::Double(Some(n)) => float(n),
        SV::String(Some(s)) => J::String(s.to_string()),
        SV::Char(Some(c)) => J::String(c.to_string()),
        SV::Bytes(Some(b)) => J::Array(b.iter().map(|x| J::from(*x)).collect()),
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
        #[cfg(feature = "with-time")]
        SV::TimeDate(Some(d)) => J::String(time_fmt::date(&d)),
        #[cfg(feature = "with-time")]
        SV::TimeTime(Some(t)) => J::String(time_fmt::time(&t)),
        #[cfg(feature = "with-time")]
        SV::TimeDateTime(Some(dt)) => J::String(time_fmt::datetime(&dt)),
        #[cfg(feature = "with-time")]
        SV::TimeDateTimeWithTimeZone(Some(dt)) => J::String(time_fmt::offset_datetime(&dt)),
        #[cfg(feature = "postgres-array")]
        SV::Array(_, Some(items)) => {
            J::Array((*items).into_iter().map(sea_value_to_json).collect())
        }
        #[cfg(feature = "with-uuid")]
        SV::Uuid(Some(u)) => J::String(u.to_string()),
        #[cfg(feature = "with-rust_decimal")]
        SV::Decimal(Some(d)) => J::String(d.to_string()),
        #[cfg(feature = "with-bigdecimal")]
        SV::BigDecimal(Some(d)) => J::String(d.to_string()),
        // Major-specific variants (sea-orm 2 native enums), `None`s and
        // types without a vld-data feature.
        other => super::compat::other_value_to_json(other),
    }
}

/// ISO 8601 strings for `time` values (the same shapes as the chrono ones).
#[cfg(feature = "with-time")]
mod time_fmt {
    use super::sea_orm::prelude::{TimeDate, TimeDateTime, TimeDateTimeWithTimeZone, TimeTime};
    use std::borrow::Borrow;

    pub(crate) fn date(d: &impl Borrow<TimeDate>) -> String {
        let d = d.borrow();
        format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
    }

    /// `HH:MM:SS[.fff[fff[fff]]]`: the fraction in 3, 6 or 9 digits, as chrono
    /// writes it, so a `time` column and a chrono column give the same text.
    pub(crate) fn time(t: &impl Borrow<TimeTime>) -> String {
        let t = t.borrow();
        let s = format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second());
        match t.nanosecond() {
            0 => s,
            n if n % 1_000_000 == 0 => format!("{s}.{:03}", n / 1_000_000),
            n if n % 1_000 == 0 => format!("{s}.{:06}", n / 1_000),
            n => format!("{s}.{n:09}"),
        }
    }

    pub(crate) fn datetime(dt: &impl Borrow<TimeDateTime>) -> String {
        let dt = dt.borrow();
        format!("{}T{}", date(&dt.date()), time(&dt.time()))
    }

    /// RFC 3339 with a `±HH:MM` offset (RFC 3339 has no offset seconds, so
    /// an offset like `+00:00:30` is written without them).
    pub(crate) fn offset_datetime(dt: &impl Borrow<TimeDateTimeWithTimeZone>) -> String {
        let dt = dt.borrow();
        let off = dt.offset();
        let secs = off.whole_seconds();
        let (sign, secs) = if secs < 0 { ('-', -secs) } else { ('+', secs) };
        format!(
            "{}T{}{sign}{:02}:{:02}",
            date(&dt.date()),
            time(&dt.time()),
            secs / 3600,
            secs / 60 % 60
        )
    }
}

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

/// Validate an [`ActiveModel`](sea_orm::ActiveModelTrait) against schema `S`.
///
/// Extracts all `Set` and `Unchanged` fields into a JSON object and runs
/// `S::vld_parse_value()`.
///
/// Use this in [`ActiveModelBehavior::before_save`](sea_orm::ActiveModelBehavior::before_save)
/// to validate before every insert/update.
///
/// ```rust,ignore
/// validate_active::<UserInput, _>(&active_model)?;
/// ```
pub fn validate_active<S, A>(model: &A) -> Result<S, VldSeaError>
where
    S: vld::schema::VldParse,
    A: sea_orm::ActiveModelTrait,
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
/// validate_model::<UserInput, _>(&input)?;
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
/// [`ActiveModelBehavior::before_save`](sea_orm::ActiveModelBehavior::before_save).
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
///         before_save::<UserInput, _>(&self)?;
///         Ok(self)
///     }
/// }
/// ```
pub fn before_save<S, A>(model: &A) -> Result<(), sea_orm::DbErr>
where
    S: vld::schema::VldParse,
    A: sea_orm::ActiveModelTrait,
{
    validate_active::<S, A>(model)
        .map(|_| ())
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Validated<S, T> — wrapper
// ---------------------------------------------------------------------------

crate::db::__vld_db_validated!(VldSeaError);

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use super::{
        Validated, VldSeaError, active_model_to_json, before_save, validate_active, validate_json,
        validate_model,
    };
    pub use vld::prelude::*;
}
