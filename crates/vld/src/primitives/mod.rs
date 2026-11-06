mod adapter;
mod any;
mod boolean;
mod bytes;
mod character;
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "chrono", feature = "jiff", feature = "time")))
)]
pub mod date;
#[cfg(feature = "decimal")]
mod decimal;
mod duration;
mod enumeration;
#[cfg(feature = "file")]
mod file;
#[cfg(feature = "net")]
mod ip_network;
mod iso;
mod json_value;
mod literal;
mod number;
mod path;
mod socket_addr;
mod string;

pub use adapter::{ZEnumOf, ZFromSerde, ZFromStr};
pub use any::{ZAny, ZNever, ZNull};
pub use boolean::ZBoolean;
pub use bytes::ZBytes;
pub use character::ZChar;
// Shortcut aliases, only when exactly one date backend is enabled.
#[cfg(all(feature = "chrono", not(feature = "jiff"), not(feature = "time")))]
pub use date::chrono::{ZDate, ZDateTime, ZTime};
#[cfg(all(feature = "jiff", not(feature = "chrono"), not(feature = "time")))]
pub use date::jiff::{ZDate, ZDateTime, ZTime};
#[cfg(all(feature = "time", not(feature = "chrono"), not(feature = "jiff")))]
pub use date::time::{ZDate, ZDateTime, ZTime};
#[cfg(feature = "decimal")]
#[cfg_attr(docsrs, doc(cfg(feature = "decimal")))]
pub use decimal::ZDecimal;
pub use duration::ZDuration;
pub use enumeration::ZEnum;
#[cfg(feature = "file")]
#[cfg_attr(docsrs, doc(cfg(feature = "file")))]
pub use file::{FileStorage, ValidatedFile, ZFile};
#[cfg(feature = "net")]
#[cfg_attr(docsrs, doc(cfg(feature = "net")))]
pub use ip_network::ZIpNetwork;
pub use json_value::ZJsonValue;
pub use literal::{IntoLiteral, ZLiteral};
pub use number::{IntType, ZF32, ZInt, ZInteger, ZNumber};
pub use path::ZPath;
pub use socket_addr::ZSocketAddr;
pub use string::ZString;

use serde_json::Value;

use crate::error::{IssueCode, VldError, issue_error, value_type_name};

/// A `with_messages` closure: check key → replacement message.
pub(crate) type Messages = Option<std::sync::Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>;

/// Build the standard `invalid_type` error, honouring a `.type_error(...)` override.
pub(crate) fn type_error(custom: &Option<String>, expected: &str, value: &Value) -> VldError {
    issue_error(
        IssueCode::InvalidType {
            expected: expected.to_string(),
            received: value_type_name(value),
        },
        custom.as_ref(),
        value,
    )
}

/// JSON equality where numbers compare by numeric value (`1 == 1.0`).
pub(crate) fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_f64() || y.is_f64() {
                x.as_f64() == y.as_f64()
            } else {
                x.as_i64() == y.as_i64() && x.as_u64() == y.as_u64()
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| json_eq(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

/// Write `min*`/`max*` length keywords; when several bounds apply the
/// strictest one wins.
#[cfg(feature = "openapi")]
pub(crate) fn length_schema(
    schema: &mut Value,
    keys: (&str, &str),
    mins: impl IntoIterator<Item = usize>,
    maxs: impl IntoIterator<Item = usize>,
) {
    if let Some(n) = mins.into_iter().max() {
        schema[keys.0] = serde_json::json!(n);
    }
    if let Some(n) = maxs.into_iter().min() {
        schema[keys.1] = serde_json::json!(n);
    }
}
