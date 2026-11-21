//! # `vld_data::csv` — validated CSV import for `vld`
//!
//! Read CSV records (the `csv` crate) into `vld` types, every row validated,
//! every failing row reported with its line:
//!
//! - [`rows`] — an iterator of `Result<T, RowError>`, one item per record;
//! - [`validate_all`] — every row, or every failing row.
//!
//! The header row names the fields: each record becomes a JSON object
//! `{header: cell}`. Cells are text, coerced the way query strings are
//! ([`vld::schema::coerce_text`] + [`vld::schema::parse_coerced`]): `42` is a
//! number for `vld::number()`, and stays `"42"` for `vld::string()`. An empty
//! cell is `""`; use `.blank_as(value)` for a column that may be blank.
//! Header names are used as they are (configure trimming on the
//! `csv::ReaderBuilder`). A record shorter than the header row lacks the
//! missing keys (`missing_field`) when the reader is `flexible(true)`;
//! otherwise any record of another length is a [`RowErrorKind::Csv`] error.
//! A malformed record does not stop the iteration; an I/O error does.
//!
//! Feature `csv` (csv 1.x).
//!
//! ```
//! use vld_data::csv::{RowErrorKind, rows, validate_all};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name: String => vld::string().min(2),
//!         pub age: i64 => vld::number().int().min(18),
//!     }
//! }
//!
//! let data = "name,age\nAlice,30\nB,12\n";
//! let errors = validate_all::<User, _>(csv::Reader::from_reader(data.as_bytes())).unwrap_err();
//! assert_eq!(errors.len(), 1);
//! assert_eq!(errors[0].line, 3);
//! assert!(matches!(&errors[0].error, RowErrorKind::Validation(e) if e.issues.len() == 2));
//!
//! let first = rows::<User, _>(csv::Reader::from_reader(data.as_bytes())).next().unwrap();
//! assert_eq!(first.unwrap().age, 30);
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/csv>

use ::csv::{Reader, StringRecord};
use serde_json::{Map, Value};
use std::fmt;
use std::io::Read;
use std::marker::PhantomData;
use vld::error::{IssueCode, VldError};
use vld::schema::{VldParse, coerce_text, parse_coerced};

pub use vld;

/// A record that could not be read or failed validation.
#[derive(Debug)]
pub struct RowError {
    /// 1-based line where the record starts (the header row is line 1).
    pub line: u64,
    /// The record's cells, when the record was read.
    pub record: Option<Vec<String>>,
    /// Why the row failed.
    pub error: RowErrorKind,
}

/// Why a row failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum RowErrorKind {
    /// The record failed schema validation (or the reader has no header row).
    Validation(VldError),
    /// The record could not be read: invalid UTF-8, a field count different
    /// from the header row, an I/O error.
    Csv(::csv::Error),
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.error {
            RowErrorKind::Validation(e) => write!(f, "line {}: validation error: {e}", self.line),
            RowErrorKind::Csv(e) => write!(f, "line {}: CSV error: {e}", self.line),
        }
    }
}

impl std::error::Error for RowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.error {
            RowErrorKind::Validation(e) => Some(e),
            RowErrorKind::Csv(e) => Some(e),
        }
    }
}

/// Iterator returned by [`rows`].
pub struct Rows<T, R> {
    reader: Reader<R>,
    headers: Option<Vec<String>>,
    record: StringRecord,
    done: bool,
    _row: PhantomData<fn() -> T>,
}

impl<T, R> fmt::Debug for Rows<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rows")
            .field("headers", &self.headers)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

/// Read the records of `reader` as `T`, one `Result` per record.
///
/// The reader must have a header row (`has_headers(true)`, the default);
/// without one the only item is a `Validation` error.
pub fn rows<T: VldParse, R: Read>(reader: Reader<R>) -> Rows<T, R> {
    Rows {
        reader,
        headers: None,
        record: StringRecord::new(),
        done: false,
        _row: PhantomData,
    }
}

/// Read every record of `reader` as `T`: all rows, or every failing row.
pub fn validate_all<T: VldParse, R: Read>(reader: Reader<R>) -> Result<Vec<T>, Vec<RowError>> {
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    for row in rows::<T, R>(reader) {
        match row {
            Ok(v) if errors.is_empty() => ok.push(v),
            Ok(_) => {}
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(ok)
    } else {
        Err(errors)
    }
}

impl<T: VldParse, R: Read> Rows<T, R> {
    fn fail(line: u64, error: RowErrorKind) -> Option<Result<T, RowError>> {
        Some(Err(RowError {
            line,
            record: None,
            error,
        }))
    }

    fn csv_error(&mut self, e: ::csv::Error) -> Option<Result<T, RowError>> {
        let line = e
            .position()
            .map_or_else(|| self.reader.position().line(), |p| p.line());
        self.done |= e.is_io_error();
        Self::fail(line, RowErrorKind::Csv(e))
    }
}

impl<T: VldParse, R: Read> Iterator for Rows<T, R> {
    type Item = Result<T, RowError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.headers.is_none() {
            if !self.reader.has_headers() {
                self.done = true;
                let e = VldError::single(IssueCode::ParseError, "CSV reader has no header row");
                return Self::fail(1, RowErrorKind::Validation(e));
            }
            match self.reader.headers() {
                Ok(h) => self.headers = Some(h.iter().map(str::to_owned).collect()),
                Err(e) => {
                    self.done = true;
                    return Self::fail(1, RowErrorKind::Csv(e));
                }
            }
        }
        match self.reader.read_record(&mut self.record) {
            Ok(false) => {
                self.done = true;
                return None;
            }
            Ok(true) => {}
            Err(e) => return self.csv_error(e),
        }
        let headers = self.headers.as_deref().unwrap_or_default();
        let object: Map<String, Value> = headers
            .iter()
            .zip(self.record.iter())
            .map(|(h, cell)| (h.clone(), coerce_text(cell)))
            .collect();
        let line = self.record.position().map_or(0, |p| p.line());
        Some(
            parse_coerced::<T>(&Value::Object(object)).map_err(|e| RowError {
                line,
                record: Some(self.record.iter().map(str::to_owned).collect()),
                error: RowErrorKind::Validation(e),
            }),
        )
    }
}

/// Common imports: [`rows`], [`validate_all`], the error types and
/// `vld::prelude`.
pub mod prelude {
    pub use super::{RowError, RowErrorKind, Rows, rows, validate_all};
    pub use vld::prelude::*;
}
