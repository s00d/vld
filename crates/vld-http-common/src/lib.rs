//! # vld-http-common — Shared HTTP helpers for `vld` web integrations
//!
//! This crate provides common utility functions used by `vld-axum`,
//! `vld-actix`, `vld-rocket`, `vld-poem`, and `vld-warp`.
//!
//! **Not intended for direct use by end users** — import via the
//! framework-specific crate instead.

/// Coerce a raw string value into a typed JSON value.
///
/// - `""` → `Null`
/// - `"true"` / `"false"` (case-insensitive) → `Bool`
/// - `"null"` (case-insensitive) → `Null`
/// - Integer-looking → `Number` (i64)
/// - Float-looking → `Number` (f64)
/// - Everything else → `String`
pub fn coerce_value(raw: &str) -> serde_json::Value {
    if raw.is_empty() {
        return serde_json::Value::Null;
    }

    if raw.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if raw.eq_ignore_ascii_case("null") {
        return serde_json::Value::Null;
    }

    if let Ok(n) = raw.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }

    if let Ok(f) = raw.parse::<f64>() {
        if f.is_finite() {
            if let Some(n) = serde_json::Number::from_f64(f) {
                return serde_json::Value::Number(n);
            }
        }
    }

    serde_json::Value::String(raw.to_string())
}

/// Parse a URL query string into a `serde_json::Map`.
///
/// Each `key=value` pair is URL-decoded and the value is coerced via
/// [`coerce_value`]. Empty pairs are skipped.
pub fn parse_query_string(query: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();

    if query.is_empty() {
        return map;
    }

    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, raw_value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };

        let key = url_decode(key);
        let raw_value = url_decode(raw_value);

        map.insert(key, coerce_value(&raw_value));
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
/// Cookie names are used as-is. Values are coerced via [`coerce_value`].
pub fn cookies_to_json(cookie_header: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    if cookie_header.is_empty() {
        return serde_json::Value::Object(map);
    }

    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if cookie.is_empty() {
            continue;
        }
        let (name, value) = match cookie.split_once('=') {
            Some((n, v)) => (n.trim(), v.trim()),
            None => (cookie.trim(), ""),
        };
        map.insert(name.to_string(), coerce_value(value));
    }

    serde_json::Value::Object(map)
}

/// Format a [`VldError`](vld::error::VldError) into a JSON array of issues.
///
/// Each issue is `{ "path": "...", "message": "..." }`.
pub fn format_issues(err: &vld::error::VldError) -> Vec<serde_json::Value> {
    err.issues
        .iter()
        .map(|i| {
            let path: String = i
                .path
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(".");
            serde_json::json!({
                "path": path,
                "message": i.message,
            })
        })
        .collect()
}

/// Format a [`VldError`](vld::error::VldError) into a JSON object with
/// `"error"` and `"issues"` keys — ready to be sent as a 422 response body.
pub fn format_vld_error(err: &vld::error::VldError) -> serde_json::Value {
    serde_json::json!({
        "error": "Validation failed",
        "issues": format_issues(err),
    })
}

/// Format issues with an additional `"code"` key from
/// [`IssueCode::key()`](vld::error::IssueCode::key).
///
/// Used by axum/actix where the error response includes `code`.
pub fn format_issues_with_code(err: &vld::error::VldError) -> Vec<serde_json::Value> {
    err.issues
        .iter()
        .map(|issue| {
            let path: String = issue
                .path
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("");
            serde_json::json!({
                "path": path,
                "message": issue.message,
                "code": issue.code.key(),
            })
        })
        .collect()
}

/// Minimal percent-decode for URL query parameters.
///
/// Handles `%XX` hex encoding and `+` → space conversion.
pub fn url_decode(input: &str) -> String {
    let s = input.replace('+', " ");
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract parameter names from a route pattern like `/users/{id}/posts/{post_id}`.
pub fn extract_path_param_names(pattern: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let name: String = chars.by_ref().take_while(|&c| c != '}').collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}
