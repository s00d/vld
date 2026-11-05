mod any;
mod boolean;
mod bytes;
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
mod date;
#[cfg(feature = "decimal")]
mod decimal;
#[cfg(feature = "std")]
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
#[cfg(feature = "std")]
mod path;
#[cfg(feature = "std")]
mod socket_addr;
mod string;

pub use any::ZAny;
pub use boolean::ZBoolean;
pub use bytes::ZBytes;
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
pub use date::{ZDate, ZDateTime};
#[cfg(feature = "decimal")]
pub use decimal::ZDecimal;
#[cfg(feature = "std")]
pub use duration::ZDuration;
pub use enumeration::ZEnum;
#[cfg(feature = "file")]
pub use file::{FileStorage, ValidatedFile, ZFile};
#[cfg(feature = "net")]
pub use ip_network::ZIpNetwork;
pub use json_value::ZJsonValue;
pub use literal::{IntoLiteral, ZLiteral};
pub use number::{ZInt, ZNumber};
#[cfg(feature = "std")]
pub use path::ZPath;
#[cfg(feature = "std")]
pub use socket_addr::ZSocketAddr;
pub use string::ZString;

use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};

/// Build the standard "invalid type" error, honouring a `.type_error(...)` override.
///
/// `expected` goes into [`IssueCode::InvalidType`]; `label` is used in the
/// default message `Expected {label}, received {type}`.
pub(crate) fn type_error(
    custom: &Option<String>,
    expected: &str,
    label: &str,
    value: &Value,
) -> VldError {
    let received = value_type_name(value);
    let msg = custom
        .clone()
        .unwrap_or_else(|| format!("Expected {}, received {}", label, received));
    VldError::single_with_value(
        IssueCode::InvalidType {
            expected: expected.to_string(),
            received,
        },
        msg,
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
