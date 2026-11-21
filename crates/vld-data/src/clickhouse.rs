//! # `vld_data::clickhouse` — ClickHouse (clickhouse-rs) for `vld`
//!
//! Validate rows against a `vld` schema around the
//! `clickhouse` client:
//!
//! - [`insert_validated`] — validate a row, then `Insert::write` it;
//! - [`fetch_all_validated`] — `Query::fetch_all`, then validate every row;
//! - [`validate_row`] / [`validate_rows`] — the check on its own.
//!
//! Rows are checked through `serde`: a row is serialized to JSON (as
//! `serde_json` sees it, not as RowBinary) and validated against `S`. Row
//! types are owned structs with `#[derive(clickhouse::Row, Serialize,
//! Deserialize)]`.
//!
//! Feature `clickhouse` (clickhouse 0.15, needs Rust 1.89; vld-data enables
//! none of its default features).
//!
//! ```no_run
//! use vld_data::clickhouse::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct EventSchema {
//!         pub name: String => vld::string().min(1),
//!         pub value: i64 => vld::number().int().min(0),
//!     }
//! }
//!
//! #[derive(clickhouse::Row, serde::Serialize, serde::Deserialize)]
//! struct Event { name: String, value: i64 }
//!
//! # async fn run(client: clickhouse::Client) -> Result<(), VldClickhouseError> {
//! let mut insert = client.insert::<Event>("events").await?;
//! insert_validated::<EventSchema, _>(&mut insert, &Event { name: "click".into(), value: 1 }).await?;
//! insert.end().await?;
//!
//! let rows: Vec<Event> =
//!     fetch_all_validated::<EventSchema, _>(client.query("SELECT ?fields FROM events")).await?;
//! # let _ = rows; Ok(()) }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/clickhouse>

use ::clickhouse::insert::Insert;
use ::clickhouse::query::Query;
use ::clickhouse::{RowOwned, RowRead, RowWrite};
use std::fmt;

pub use vld;

/// Validate one row against schema `S`.
pub fn validate_row<S, T>(row: &T) -> Result<(), VldClickhouseError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json =
        serde_json::to_value(row).map_err(|e| VldClickhouseError::Serialization(e.to_string()))?;
    S::vld_parse_value(&json)
        .map(|_| ())
        .map_err(VldClickhouseError::Validation)
}

/// Validate a batch of rows against schema `S`.
///
/// Returns the index and error of the first invalid row.
pub fn validate_rows<S, T>(rows: &[T]) -> Result<(), (usize, VldClickhouseError)>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    for (i, row) in rows.iter().enumerate() {
        validate_row::<S, T>(row).map_err(|e| (i, e))?;
    }
    Ok(())
}

/// Validate `row` against schema `S`, then write it to `insert`.
///
/// An invalid row is not written and the `INSERT` stays usable.
pub async fn insert_validated<S, T>(
    insert: &mut Insert<T>,
    row: &T,
) -> Result<(), VldClickhouseError>
where
    S: vld::schema::VldParse,
    T: RowOwned + RowWrite + serde::Serialize,
{
    validate_row::<S, T>(row)?;
    Ok(insert.write(row).await?)
}

/// Run `query` with `fetch_all`, then validate every row against schema `S`.
///
/// The first invalid row is [`VldClickhouseError::InvalidRow`] with its index.
pub async fn fetch_all_validated<S, T>(query: Query) -> Result<Vec<T>, VldClickhouseError>
where
    S: vld::schema::VldParse,
    T: RowOwned + RowRead + serde::Serialize,
{
    let rows = query.fetch_all::<T>().await?;
    validate_rows::<S, T>(&rows).map_err(|(index, e)| VldClickhouseError::InvalidRow {
        index,
        error: Box::new(e),
    })?;
    Ok(rows)
}

/// Error type of the clickhouse module.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldClickhouseError {
    /// The row failed schema validation.
    Validation(vld::error::VldError),
    /// The row could not be serialized to JSON for validation.
    Serialization(String),
    /// A fetched row failed: its index and why.
    InvalidRow {
        /// Index of the row in the result.
        index: usize,
        /// `Validation` or `Serialization`.
        error: Box<VldClickhouseError>,
    },
    /// The ClickHouse client failed.
    Clickhouse(::clickhouse::error::Error),
}

impl fmt::Display for VldClickhouseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldClickhouseError::Validation(e) => write!(f, "Validation error: {e}"),
            VldClickhouseError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldClickhouseError::InvalidRow { index, error } => write!(f, "Row {index}: {error}"),
            VldClickhouseError::Clickhouse(e) => write!(f, "ClickHouse error: {e}"),
        }
    }
}

impl std::error::Error for VldClickhouseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldClickhouseError::Validation(e) => Some(e),
            VldClickhouseError::InvalidRow { error, .. } => Some(error.as_ref()),
            VldClickhouseError::Clickhouse(e) => Some(e),
            VldClickhouseError::Serialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldClickhouseError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

impl From<::clickhouse::error::Error> for VldClickhouseError {
    fn from(e: ::clickhouse::error::Error) -> Self {
        Self::Clickhouse(e)
    }
}

/// Common imports: the functions, `VldClickhouseError` and `vld::prelude`.
pub mod prelude {
    pub use super::{
        VldClickhouseError, fetch_all_validated, insert_validated, validate_row, validate_rows,
    };
    pub use vld::prelude::*;
}
