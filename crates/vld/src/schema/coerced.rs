//! Schema-aware fallback for values coerced from text (query strings, forms,
//! headers, cookies, path params, env-based config).

use serde_json::Value;

use crate::error::{IssueCode, PathSegment, VldError};
use crate::schema::VldParse;

/// Coerce a raw text value (query string, form field, path param, header,
/// cookie, environment variable) into JSON, for [`parse_coerced`].
///
/// Coercion never loses information: a value is only converted when the
/// conversion round-trips to exactly the same text.
///
/// - `"true"` / `"false"` (lowercase only) → `Bool`
/// - canonical integers (`0`, `42`, `-7`; no `+`, no leading zeros) that fit
///   `i64`/`u64` → `Number`
/// - canonical decimals (`3.14`, `-0.5`; no exponent, no trailing zeros, no
///   precision loss in `f64`) → `Number`
/// - everything else — including `""`, `"null"`, `"01234"`, `"+1555"`,
///   `"1e3"`, `"TRUE"` and ids beyond `u64` — stays a `String`
///
/// Use `vld::string().coerce()` / `vld::number().coerce()` in schemas when a
/// field must accept both forms (e.g. a string field that may look numeric).
/// [`parse_coerced`] turns a coerced number / boolean back into its text
/// where the schema expects a string, so the two together follow the schema.
///
/// # Example
/// ```
/// use serde_json::json;
/// use vld::schema::coerce_text;
///
/// assert_eq!(coerce_text("42"), json!(42));
/// assert_eq!(coerce_text("-0.5"), json!(-0.5));
/// assert_eq!(coerce_text("true"), json!(true));
/// assert_eq!(coerce_text("01234"), json!("01234"));
/// assert_eq!(coerce_text("null"), json!("null"));
/// ```
pub fn coerce_text(raw: &str) -> Value {
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if is_canonical_number(raw) {
        if !raw.contains('.') {
            if let Ok(n) = raw.parse::<i64>() {
                return Value::Number(n.into());
            }
            if let Ok(n) = raw.parse::<u64>() {
                return Value::Number(n.into());
            }
        } else if let Ok(f) = raw.parse::<f64>() {
            // `f64` Display is the shortest round-trip form: `1.50` → `1.5`,
            // `0.1000000000000000001` → `0.1`; both stay strings.
            if f.is_finite()
                && f.to_string() == raw
                && let Some(n) = serde_json::Number::from_f64(f)
            {
                return Value::Number(n);
            }
        }
    }
    Value::String(raw.to_string())
}

/// `-?(0|[1-9][0-9]*)(\.[0-9]+)?`, excluding `-0`.
fn is_canonical_number(s: &str) -> bool {
    let negative = s.starts_with('-');
    let digits = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    let all_digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(int) || (int.len() > 1 && int.starts_with('0')) {
        return false;
    }
    if frac.is_some_and(|f| !all_digits(f)) {
        return false;
    }
    !(negative && frac.is_none() && int == "0")
}

/// Parse a value whose leaves were coerced from text (`"42"` → `42`,
/// `"true"` → `true`), undoing the coercion where the schema disagrees.
///
/// The value is parsed as is first. When that fails, every issue that is
/// - `invalid_type` expecting `string` at a coerced number / boolean → the
///   leaf is turned back into its text (`?q=123` into `vld::string()` is `"123"`);
/// - `invalid_type` expecting `array` at a scalar → the leaf is wrapped into a
///   one-element array (`?tags=a` into `Vec<String>` is `["a"]`)
///
/// is repaired and the value parsed again (a few rounds, so `?tags=1` into
/// `Vec<String>` becomes `["1"]`). The last error is returned when nothing
/// more can be repaired.
///
/// Text is recovered from the coerced value itself, so the coercion must be
/// lossless (as [`coerce_text`] is). For lossy sources (e.g.
/// config-rs `try_parsing`, where `"01234"` becomes `1234`) the recovered
/// text is the canonical form of the number.
///
/// # Example
/// ```
/// use serde_json::json;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct Q {
///         q: String => vld::string(),
///         age: i64 => vld::number().int(),
///         tags: Vec<String> => vld::array(vld::string()),
///     }
/// }
///
/// let q: Q = vld::schema::parse_coerced(&json!({"q": 123, "age": 30, "tags": "a"})).unwrap();
/// assert_eq!((q.q.as_str(), q.age, q.tags), ("123", 30, vec!["a".to_string()]));
/// ```
pub fn parse_coerced<T: VldParse>(value: &Value) -> Result<T, VldError> {
    let mut err = match T::vld_parse_value(value) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    let mut current = value.clone();
    // ponytail: fixed round cap; each round repairs one nesting level
    // (wrap, then un-coerce the wrapped item).
    for _ in 0..4 {
        if !repair(&mut current, &err) {
            break;
        }
        match T::vld_parse_value(&current) {
            Ok(v) => return Ok(v),
            Err(e) => err = e,
        }
    }
    Err(err)
}

/// Apply the repairs for `err` to `value`; `true` when anything changed.
fn repair(value: &mut Value, err: &VldError) -> bool {
    let mut changed = false;
    for issue in &err.issues {
        let IssueCode::InvalidType { expected, .. } = &issue.code else {
            continue;
        };
        let Some(leaf) = pointer_mut(value, &issue.path) else {
            continue;
        };
        match (expected.as_str(), &*leaf) {
            ("string", Value::Number(_) | Value::Bool(_)) => {
                *leaf = Value::String(uncoerce(leaf));
                changed = true;
            }
            ("array", Value::String(_) | Value::Number(_) | Value::Bool(_)) => {
                *leaf = Value::Array(vec![leaf.take()]);
                changed = true;
            }
            _ => {}
        }
    }
    changed
}

/// Text a coerced scalar came from (exact for lossless coercion).
fn uncoerce(v: &Value) -> String {
    match v {
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => i.to_string(),
            (_, Some(u), _) => u.to_string(),
            // Rust `Display` is what the coercion round-trips against.
            (_, _, Some(f)) => f.to_string(),
            _ => n.to_string(),
        },
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn pointer_mut<'a>(mut value: &'a mut Value, path: &[PathSegment]) -> Option<&'a mut Value> {
    for seg in path {
        value = match seg {
            PathSegment::Field(k) => value.as_object_mut()?.get_mut(k)?,
            PathSegment::Index(i) => value.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(value)
}
