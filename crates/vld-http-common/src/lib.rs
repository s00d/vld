//! # vld-http-common — Shared HTTP helpers for `vld` web integrations
//!
//! This crate provides the common parsing helpers and the shared
//! [`VldHttpError`] type used by `vld-actix`, `vld-axum`, `vld-ntex`,
//! `vld-poem`, `vld-rama`, `vld-rocket`, `vld-salvo`, `vld-tower`,
//! `vld-warp` and `vld-tonic`.
//!
//! **Not intended for direct use by end users** — import via the
//! framework-specific crate instead.
//!
//! # Value semantics (all adapters)
//!
//! - Query strings and form bodies: repeated keys (`a=1&a=2`) and `a[]=1`
//!   become arrays; `?flag` and `?x=` become `""`.
//! - Values are coerced with [`coerce_value`], which never loses information.
//! - Coerced sources are validated with [`parse_coerced`]: a coerced number /
//!   boolean is turned back into text where the schema expects a string
//!   (`?q=123` into `vld::string()` is `"123"`), and a single value is
//!   wrapped into an array where the schema expects one (`?tags=a`).
//! - JSON body extractors require a JSON `Content-Type` (`application/json`
//!   or `application/*+json`, see [`is_json_content_type`]); a missing or
//!   different type is **415** `{"error": "Unsupported Media Type", "message": ...}`.
//! - Malformed bodies (JSON, non-UTF-8 forms) → **422** with the
//!   [`ValidationErrorBody`] shape; oversized bodies → **413**
//!   `{"error": "Payload too large"}`.

pub use vld::schema::parse_coerced;

/// Default max request body size (2 MiB — same as axum `DefaultBodyLimit`).
///
/// Used by every adapter that reads the body itself (axum, poem, rama, tower,
/// warp, rocket form fallback). Adapters that delegate body reading to the
/// framework (actix, ntex, salvo, rocket) honour the framework's own,
/// user-configurable limit instead.
pub const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Shared adapter conformance suite (test support, `conformance` feature).
#[cfg(feature = "conformance")]
pub mod conformance;

/// Coerce a raw string value (query, form, path, header, cookie) into JSON.
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
pub fn coerce_value(raw: &str) -> serde_json::Value {
    use serde_json::Value;
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
            if f.is_finite() && f.to_string() == raw {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    return Value::Number(n);
                }
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

/// Parse a URL query string (or `application/x-www-form-urlencoded` body)
/// into a `serde_json::Map`.
///
/// - keys and values are form-decoded ([`url_decode`]) and values coerced via
///   [`coerce_value`];
/// - a key without `=` (`?flag`) yields `""`, like `URLSearchParams`;
/// - a repeated key (`a=1&a=2`) yields an array `[1, 2]`;
/// - a key ending in `[]` (`a[]=1`) always yields an array under `a`.
pub fn parse_query_string(query: &str) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::Value;
    let mut map = serde_json::Map::new();

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let mut key = url_decode(key);
        let value = coerce_value(&url_decode(raw_value));

        let force_array = key.ends_with("[]");
        if force_array {
            key.truncate(key.len() - 2);
        }

        match map.get_mut(&key) {
            Some(Value::Array(items)) => items.push(value),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, value]);
            }
            None if force_array => {
                map.insert(key, Value::Array(vec![value]));
            }
            None => {
                map.insert(key, value);
            }
        }
    }

    map
}

/// Parse a URL query string into a `serde_json::Value::Object`.
///
/// Convenience wrapper around [`parse_query_string`].
pub fn query_string_to_json(query: &str) -> serde_json::Value {
    serde_json::Value::Object(parse_query_string(query))
}

/// Build a JSON object from a `Cookie` header value.
///
/// Cookie names are used as-is. Values are unquoted (`"abc"` → `abc`),
/// percent-decoded (`+` is kept) and coerced via [`coerce_value`].
pub fn cookies_to_json(cookie_header: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if cookie.is_empty() {
            continue;
        }
        let (name, value) = cookie.split_once('=').unwrap_or((cookie, ""));
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        map.insert(
            name.trim().to_string(),
            coerce_value(&percent_decode(value)),
        );
    }

    serde_json::Value::Object(map)
}

/// Build a JSON object from HTTP header name/value pairs.
///
/// - names are lowercased and `-` becomes `_` (`X-Request-Id` → `x_request_id`);
/// - non-UTF-8 bytes become `U+FFFD` instead of silently dropping the header;
/// - repeated headers are joined with `", "` (RFC 9110 §5.3);
/// - the joined value is coerced via [`coerce_value`].
pub fn headers_to_json<'a, I>(headers: I) -> serde_json::Value
where
    I: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let mut map = serde_json::Map::new();
    let mut raw: Vec<(String, String)> = Vec::new();
    for (name, value) in headers {
        let key = name.to_ascii_lowercase().replace('-', "_");
        let value = String::from_utf8_lossy(value);
        match raw.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => {
                v.push_str(", ");
                v.push_str(&value);
            }
            None => raw.push((key, value.into_owned())),
        }
    }
    for (k, v) in raw {
        map.insert(k, coerce_value(&v));
    }
    serde_json::Value::Object(map)
}

/// Build a JSON object from already-decoded path parameters, coercing each
/// value via [`coerce_value`].
pub fn path_params_to_json<'a, I>(params: I) -> serde_json::Value
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    serde_json::Value::Object(
        params
            .into_iter()
            .map(|(k, v)| (k.to_string(), coerce_value(v)))
            .collect(),
    )
}

/// Parse an `application/x-www-form-urlencoded` body.
///
/// Fails with a 422 parse error when the body is not valid UTF-8.
pub fn parse_form_body(body: &[u8]) -> Result<serde_json::Value, VldHttpError> {
    let s = std::str::from_utf8(body)
        .map_err(|_| VldHttpError::parse("Form body is not valid UTF-8"))?;
    Ok(query_string_to_json(s))
}

/// Parse a JSON body; malformed JSON is a 422 parse error.
pub fn parse_json_body(body: &[u8]) -> Result<serde_json::Value, VldHttpError> {
    serde_json::from_slice(body).map_err(|e| VldHttpError::parse(format!("Invalid JSON: {e}")))
}

/// `true` for `application/json` and any `application/*+json` media type
/// (case-insensitive; parameters such as `; charset=utf-8` are ignored).
pub fn is_json_content_type(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    essence == "application/json"
        || (essence.starts_with("application/") && essence.ends_with("+json"))
}

/// Check the `Content-Type` of a JSON body: `Ok` for JSON media types
/// ([`is_json_content_type`]), otherwise a **415** error (also when the
/// header is missing or not valid UTF-8).
pub fn require_json_content_type(content_type: Option<&str>) -> Result<(), VldHttpError> {
    match content_type {
        Some(ct) if is_json_content_type(ct) => Ok(()),
        _ => Err(VldHttpError::unsupported_media_type()),
    }
}

// ---------------------------------------------------------------------------
// Shared error type
// ---------------------------------------------------------------------------

/// Framework-independent error produced by all `vld` HTTP adapters.
///
/// | Constructor | Status | Body |
/// |---|---|---|
/// | [`validation`](Self::validation) / [`parse`](Self::parse) | 422 | [`ValidationErrorBody`] |
/// | [`payload_too_large`](Self::payload_too_large) | 413 | [`ErrorBody`] |
/// | [`unsupported_media_type`](Self::unsupported_media_type) | 415 | [`ErrorWithMessage`] |
///
/// Adapters only convert [`status`](Self::status) and [`body`](Self::body)
/// into their framework's response type.
#[derive(Debug, Clone)]
pub struct VldHttpError {
    status: u16,
    error: vld::error::VldError,
}

impl VldHttpError {
    /// 422 — schema validation failed.
    pub fn validation(error: vld::error::VldError) -> Self {
        Self { status: 422, error }
    }

    /// 422 — the input could not be parsed (malformed JSON, bad UTF-8, ...).
    pub fn parse(message: impl Into<String>) -> Self {
        Self::validation(vld::error::VldError::single(
            vld::error::IssueCode::ParseError,
            message.into(),
        ))
    }

    /// 413 — the body exceeds the size limit.
    ///
    /// [`error`](Self::error) holds a single `ParseError` issue
    /// `"Payload too large"` (also what `Display` prints).
    pub fn payload_too_large() -> Self {
        Self {
            status: 413,
            error: vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                "Payload too large",
            ),
        }
    }

    /// 415 — a JSON body was sent without a JSON `Content-Type`.
    ///
    /// [`error`](Self::error) holds a single `ParseError` issue with
    /// [`UNSUPPORTED_MEDIA_TYPE_MESSAGE`].
    pub fn unsupported_media_type() -> Self {
        Self {
            status: 415,
            error: vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                UNSUPPORTED_MEDIA_TYPE_MESSAGE,
            ),
        }
    }

    /// HTTP status code (422, 413 or 415).
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The underlying validation error.
    pub fn error(&self) -> &vld::error::VldError {
        &self.error
    }

    /// Consume and return the underlying validation error.
    pub fn into_error(self) -> vld::error::VldError {
        self.error
    }

    /// JSON response body for this error.
    pub fn body(&self) -> serde_json::Value {
        match self.status {
            413 => format_payload_too_large(),
            415 => format_unsupported_media_type(),
            _ => format_vld_error(&self.error),
        }
    }
}

impl From<vld::error::VldError> for VldHttpError {
    fn from(error: vld::error::VldError) -> Self {
        Self::validation(error)
    }
}

impl std::fmt::Display for VldHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            413 => f.write_str("Payload too large"),
            415 => f.write_str("Unsupported Media Type"),
            _ => write!(f, "Validation failed: {}", self.error),
        }
    }
}

impl std::error::Error for VldHttpError {}

/// Implement `Deref`/`DerefMut` to the inner value for tuple extractors
/// `Name<T>(pub T)`.
#[doc(hidden)]
#[macro_export]
macro_rules! impl_deref {
    ($($name:ident),+ $(,)?) => {$(
        impl<T> ::std::ops::Deref for $name<T> {
            type Target = T;
            fn deref(&self) -> &T {
                &self.0
            }
        }
        impl<T> ::std::ops::DerefMut for $name<T> {
            fn deref_mut(&mut self) -> &mut T {
                &mut self.0
            }
        }
    )+};
}

// ---------------------------------------------------------------------------
// Error response schemas (defined via vld::schema!)
// ---------------------------------------------------------------------------

use serde::Serialize;

vld::schema! {
    /// Simple error body: `{ "error": "..." }`.
    ///
    /// Used for generic HTTP errors such as "Payload too large",
    /// "Not Found", "Method Not Allowed", etc.
    #[derive(Debug, Clone, Serialize)]
    pub struct ErrorBody {
        pub error: String => vld::string(),
    }
}

vld::schema! {
    /// Error body with a message: `{ "error": "...", "message": "..." }`.
    ///
    /// Returned by [`format_json_parse_error`]. The adapters report malformed
    /// JSON as a 422 [`ValidationErrorBody`] instead.
    #[derive(Debug, Clone, Serialize)]
    pub struct ErrorWithMessage {
        pub error: String   => vld::string(),
        pub message: String => vld::string(),
    }
}

vld::schema! {
    /// A single validation issue:
    /// `{ "path": "...", "message": "...", "code": "..." }` (`code` is
    /// [`IssueCode::key()`](vld::error::IssueCode::key)).
    #[derive(Debug, Clone, Serialize)]
    pub struct ValidationIssue {
        pub path: String    => vld::string(),
        pub message: String => vld::string(),
        pub code: String    => vld::string(),
    }
}

vld::schema! {
    /// A validation issue with an error code:
    /// `{ "path": "...", "message": "...", "code": "..." }`.
    ///
    /// Used by `vld-tonic` status details.
    #[derive(Debug, Clone, Serialize)]
    pub struct ValidationIssueWithCode {
        pub path: String    => vld::string(),
        pub message: String => vld::string(),
        pub code: String    => vld::string(),
    }
}

vld::schema! {
    /// Validation error response body:
    /// `{ "error": "Validation failed", "issues": [...] }`.
    #[derive(Debug, Clone, Serialize)]
    pub struct ValidationErrorBody {
        pub error: String => vld::string(),
        pub issues: Vec<ValidationIssue> => vld::array(vld::nested(ValidationIssue::parse_value)),
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// `.user.name`, `.items[0]` — spelled out instead of relying on
/// `PathSegment`'s `Display` so the wire format stays stable.
fn format_path(path: &[vld::error::PathSegment]) -> String {
    use std::fmt::Write;
    use vld::error::PathSegment;
    let mut out = String::new();
    for seg in path {
        let _ = match seg {
            PathSegment::Field(name) => write!(out, ".{name}"),
            PathSegment::Index(idx) => write!(out, "[{idx}]"),
        };
    }
    out
}

/// Format a [`VldError`](vld::error::VldError) into a list of
/// [`ValidationIssue`] structs.
pub fn format_issues(err: &vld::error::VldError) -> Vec<ValidationIssue> {
    err.issues
        .iter()
        .map(|i| ValidationIssue {
            path: format_path(&i.path),
            message: i.message.clone(),
            code: i.code.key().to_string(),
        })
        .collect()
}

/// Format a [`VldError`](vld::error::VldError) into a JSON object with
/// `"error"` and `"issues"` keys — ready to be sent as a 422 response body.
///
/// Internally constructs a [`ValidationErrorBody`] and serializes it.
pub fn format_vld_error(err: &vld::error::VldError) -> serde_json::Value {
    let body = ValidationErrorBody {
        error: "Validation failed".into(),
        issues: format_issues(err),
    };
    serde_json::to_value(body).expect("ValidationErrorBody serialization cannot fail")
}

/// Format issues with an additional `"code"` key from
/// [`IssueCode::key()`](vld::error::IssueCode::key).
///
/// Returns a list of [`ValidationIssueWithCode`] structs.
pub fn format_issues_with_code(err: &vld::error::VldError) -> Vec<ValidationIssueWithCode> {
    err.issues
        .iter()
        .map(|issue| ValidationIssueWithCode {
            path: format_path(&issue.path),
            message: issue.message.clone(),
            code: issue.code.key().to_string(),
        })
        .collect()
}

/// Decode a form/query component: `+` → space, then [`percent_decode`].
pub fn url_decode(input: &str) -> String {
    percent_decode(&input.replace('+', " "))
}

/// Percent-decode a URL component (path segment, cookie value); `+` is kept.
///
/// Only `%` followed by exactly two hex digits is decoded; anything else is
/// kept literally. Decoded bytes are read as UTF-8 (`%C3%A9` → `é`); invalid
/// sequences become `U+FFFD`.
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let hex = |i: usize| bytes.get(i).and_then(|&b| (b as char).to_digit(16));
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(h), Some(l)) = (hex(i + 1), hex(i + 2)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

// ---------------------------------------------------------------------------
// Standard HTTP error response helpers
// ---------------------------------------------------------------------------

/// Build a JSON error body for invalid JSON parse errors.
///
/// Returns `{ "error": "Invalid JSON", "message": "..." }`.
///
/// Internally constructs an [`ErrorWithMessage`] and serializes it.
pub fn format_json_parse_error(message: &str) -> serde_json::Value {
    serde_json::to_value(ErrorWithMessage {
        error: "Invalid JSON".into(),
        message: message.into(),
    })
    .expect("ErrorWithMessage serialization cannot fail")
}

/// Build a JSON error body for invalid UTF-8 payloads.
///
/// Returns `{ "error": "Invalid UTF-8" }`.
///
/// Internally constructs an [`ErrorBody`] and serializes it.
pub fn format_utf8_error() -> serde_json::Value {
    format_generic_error("Invalid UTF-8")
}

/// Build a JSON error body for payloads that exceed the size limit.
///
/// Returns `{ "error": "Payload too large" }`.
///
/// Internally constructs an [`ErrorBody`] and serializes it.
pub fn format_payload_too_large() -> serde_json::Value {
    format_generic_error("Payload too large")
}

/// Message of the 415 body and error.
pub const UNSUPPORTED_MEDIA_TYPE_MESSAGE: &str =
    "Expected Content-Type: application/json (or application/*+json)";

/// Build the JSON error body for a non-JSON `Content-Type`.
///
/// Returns `{ "error": "Unsupported Media Type", "message": "Expected Content-Type: ..." }`.
pub fn format_unsupported_media_type() -> serde_json::Value {
    serde_json::to_value(ErrorWithMessage {
        error: "Unsupported Media Type".into(),
        message: UNSUPPORTED_MEDIA_TYPE_MESSAGE.into(),
    })
    .expect("ErrorWithMessage serialization cannot fail")
}

/// Build a generic JSON error body with a custom error string.
///
/// Returns `{ "error": "<error>" }`.
///
/// Internally constructs an [`ErrorBody`] and serializes it.
pub fn format_generic_error(error: &str) -> serde_json::Value {
    serde_json::to_value(ErrorBody {
        error: error.into(),
    })
    .expect("ErrorBody serialization cannot fail")
}
