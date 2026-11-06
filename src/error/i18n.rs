//! Internationalization (i18n) support for validation error messages.
//!
//! Provides [`MessageResolver`] trait and built-in resolvers to translate
//! [`VldError`] messages using [`IssueCode::message_keys()`] and
//! [`IssueCode::params()`].
//!
//! # Example
//!
//! ```
//! use vld::prelude::*;
//! use vld::i18n::{MessageResolver, MapResolver, translate_error};
//! use std::collections::HashMap;
//!
//! let mut translations = HashMap::new();
//! translations.insert("too_small".to_string(),
//!     "Debe tener al menos {minimum} caracteres".to_string());
//! translations.insert("invalid_type".to_string(),
//!     "Se esperaba {expected}, se recibió {received}".to_string());
//! let resolver = MapResolver::new(translations);
//!
//! let schema = vld::string().min(5);
//! let err = schema.parse(r#""ab""#).unwrap_err();
//! let translated = translate_error(&err, &resolver);
//! assert!(translated.issues[0].message.contains("5"));
//! ```

#[cfg(doc)]
use crate::error::IssueCode;
use crate::error::{ValidationIssue, VldError};
use std::collections::HashMap;

/// Trait for resolving validation messages by error code key.
///
/// Implementations receive the template keys of [`IssueCode::message_keys()`]
/// (most specific first, e.g. `"too_small.string"` then `"too_small"`) and
/// return a template, or `None` to try the next key / keep the original message.
///
/// Templates can use `{param_name}` placeholders that will be filled from
/// [`IssueCode::params()`].
pub trait MessageResolver {
    /// Return a translated template for the given error code key,
    /// or `None` to keep the original message.
    fn resolve(&self, key: &str) -> Option<String>;

    /// Plural category of `n` (`"one"`, `"few"`, `"many"`, `"other"`) for
    /// templates with count variants (see [`translate_issue`]). Default:
    /// English rule ([`plural_en`]).
    fn plural_category(&self, n: u64) -> &'static str {
        plural_en(n)
    }
}

/// English / German / Spanish plural rule: `1` → `"one"`, else `"other"`.
pub fn plural_en(n: u64) -> &'static str {
    if n == 1 { "one" } else { "other" }
}

/// Russian plural rule: `"one"` (1, 21, 31, …), `"few"` (2-4, 22-24, …),
/// `"many"` (0, 5-20, 25-30, …).
pub fn plural_ru(n: u64) -> &'static str {
    let (m10, m100) = (n % 10, n % 100);
    if m10 == 1 && m100 != 11 {
        "one"
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        "few"
    } else {
        "many"
    }
}

/// Simple [`MessageResolver`] backed by a `HashMap<String, String>`.
///
/// # Example
/// ```
/// use vld::i18n::MapResolver;
/// use std::collections::HashMap;
///
/// let mut m = HashMap::new();
/// m.insert("too_small".into(), "Минимум {minimum}".into());
/// let resolver = MapResolver::new(m);
/// ```
pub struct MapResolver {
    map: HashMap<String, String>,
    plural: fn(u64) -> &'static str,
}

impl MapResolver {
    /// Resolver over `map`, with the English plural rule.
    pub fn new(map: HashMap<String, String>) -> Self {
        Self {
            map,
            plural: plural_en,
        }
    }

    /// Use another plural rule (e.g. [`plural_ru`]).
    pub fn with_plural_rule(mut self, rule: fn(u64) -> &'static str) -> Self {
        self.plural = rule;
        self
    }
}

impl MessageResolver for MapResolver {
    fn resolve(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }

    fn plural_category(&self, n: u64) -> &'static str {
        (self.plural)(n)
    }
}

/// A [`MessageResolver`] that delegates to a closure.
///
/// # Example
/// ```
/// use vld::i18n::FnResolver;
///
/// let resolver = FnResolver::new(|key| match key {
///     "too_small" => Some("Too short!".into()),
///     _ => None,
/// });
/// ```
pub struct FnResolver<F: Fn(&str) -> Option<String>> {
    f: F,
}

impl<F: Fn(&str) -> Option<String>> FnResolver<F> {
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F: Fn(&str) -> Option<String>> MessageResolver for FnResolver<F> {
    fn resolve(&self, key: &str) -> Option<String> {
        (self.f)(key)
    }
}

/// Translate a single issue using the resolver.
///
/// The resolver is asked for every key of [`IssueCode::message_keys()`],
/// most specific first (e.g. `"too_small.string"`, then `"too_small"`); the
/// first template found is filled with [`IssueCode::params()`]. Without any
/// match the original message is kept.
///
/// **Custom messages are kept**: an issue whose message is not the default
/// English one ([`ValidationIssue::has_custom_message`] — set by `*_msg`
/// builders, `with_messages`, `.message()`, `type_error()`, `refine`, …) is
/// returned unchanged. A union's `[variant N] ` prefix is preserved.
///
/// **Plurals**: for codes with a count ([`IssueCode::plural_count`]:
/// the `minimum` / `maximum` / `expected` of size checks) the key
/// `<key>_<category>` is tried before `<key>`, where the category comes from
/// [`MessageResolver::plural_category`] (`too_small.string_one`,
/// `invalid_length.string_few`, …). The plain key is the fallback, so it
/// holds the `"other"` (English) / `"many"` (Russian) form.
///
/// Type names in `invalid_type` messages are translated through the keys
/// `type.<name>` (`type.string`, `type.date`, …) when the resolver has them.
///
/// Exclusive bounds (`gt()`, `lt()`, `positive()`, …) use the
/// `"too_small_exclusive"` / `"too_big_exclusive"` keys, so an inclusive
/// "at least" template is never applied to a "greater than" check.
pub fn translate_issue(issue: &ValidationIssue, resolver: &dyn MessageResolver) -> ValidationIssue {
    let (prefix, _) = split_union_prefix(&issue.message);
    let mut out = issue.clone();
    if issue.has_custom_message() {
        return out;
    }
    let category = issue
        .code
        .plural_count()
        .map(|n| resolver.plural_category(n));
    let template = issue.code.message_keys().iter().find_map(|k| {
        category
            .filter(|c| *c != "other")
            .and_then(|c| resolver.resolve(&format!("{k}_{c}")))
            .or_else(|| resolver.resolve(k))
    });
    if let Some(template) = template {
        // Type names (`{expected}` / `{received}` of `invalid_type`) are
        // looked up as `type.<name>`.
        let message = issue
            .code
            .render_with(&template, |name, value| match (&issue.code, name) {
                (crate::error::IssueCode::InvalidType { .. }, "expected" | "received") => {
                    resolver.resolve(&format!("type.{value}"))
                }
                _ => None,
            });
        out.message = format!("{}{message}", translate_union_prefix(prefix, resolver));
    }
    out
}

/// `"[variant 2] "` → the resolver's `union_variant` template (`"[вариант 2] "`);
/// kept as is when the resolver has none.
fn translate_union_prefix(prefix: &str, resolver: &dyn MessageResolver) -> String {
    match crate::combinators::split_variant_prefix(prefix) {
        Some((n, _)) => resolver
            .resolve("union_variant")
            .map(|t| format!("{} ", t.replace("{n}", &n.to_string())))
            .unwrap_or_else(|| prefix.to_string()),
        None => prefix.to_string(),
    }
}

/// `"[variant 2] msg"` → `("[variant 2] ", "msg")`; no prefix → `("", msg)`.
pub(crate) fn split_union_prefix(message: &str) -> (&str, &str) {
    match crate::combinators::split_variant_prefix(message) {
        Some((_, rest)) => message.split_at(message.len() - rest.len()),
        None => ("", message),
    }
}

/// Translate all issues in a [`VldError`].
///
/// Returns a new `VldError` with translated messages.
///
/// # Example
/// ```
/// use vld::prelude::*;
/// use vld::i18n::{FnResolver, translate_error};
///
/// let resolver = FnResolver::new(|key| match key {
///     "too_small" => Some("Zu kurz! Mindestens {minimum} Zeichen.".into()),
///     _ => None,
/// });
///
/// let err = vld::string().min(3).parse(r#""ab""#).unwrap_err();
/// let translated = translate_error(&err, &resolver);
/// assert!(translated.issues[0].message.contains("3"));
/// ```
pub fn translate_error(error: &VldError, resolver: &dyn MessageResolver) -> VldError {
    VldError {
        issues: error
            .issues
            .iter()
            .map(|i| translate_issue(i, resolver))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Built-in translation sets
// ---------------------------------------------------------------------------

/// The default English templates: the single source of every built-in message.
///
/// Keys follow [`IssueCode::message_keys()`]; every generic key (the
/// [`IssueCode::key()`] of each built-in code, plus `too_small_exclusive`,
/// `too_big_exclusive` and `custom`) is present.
const ENGLISH: &[(&str, &str)] = &[
    ("invalid_type", "Expected {expected}, received {received}"),
    // Labels of the `invalid_type` type keys that are not plain words
    // (`string`, `number`, … render as themselves).
    ("type.date", "date string (YYYY-MM-DD)"),
    ("type.datetime", "datetime string"),
    ("type.time", "time string (HH:MM:SS)"),
    (
        "type.duration",
        "duration as seconds or string (`PT15S`, `1.5s`, `250ms`)",
    ),
    ("type.decimal", "decimal as string or number"),
    ("type.path", "path string"),
    ("type.file", "file path or object with a string `path`"),
    ("type.socket_addr", "socket address string"),
    ("type.ip_network", "CIDR string"),
    ("type.bytes", "bytes array"),
    ("type.bytes_or_string", "bytes array or encoded string"),
    ("type.byte", "byte (integer 0..=255)"),
    // Lower bounds
    ("too_small", "Value must be at least {minimum}"),
    (
        "too_small.string",
        "String must be at least {minimum} characters",
    ),
    (
        "too_small.string_one",
        "String must be at least {minimum} character",
    ),
    ("too_small.number", "Number must be at least {minimum}"),
    ("too_small.decimal", "Decimal must be at least {minimum}"),
    (
        "too_small.array",
        "Array must have at least {minimum} elements",
    ),
    (
        "too_small.array_one",
        "Array must have at least {minimum} element",
    ),
    (
        "too_small.set",
        "Set must have at least {minimum} unique elements",
    ),
    (
        "too_small.set_one",
        "Set must have at least {minimum} unique element",
    ),
    (
        "too_small.record",
        "Record must have at least {minimum} keys",
    ),
    (
        "too_small.record_one",
        "Record must have at least {minimum} key",
    ),
    (
        "too_small.object",
        "Object must contain at least {minimum} keys",
    ),
    (
        "too_small.object_one",
        "Object must contain at least {minimum} key",
    ),
    ("too_small.bytes", "Bytes length must be at least {minimum}"),
    ("too_small.date", "Date must be on or after {minimum}"),
    (
        "too_small.datetime",
        "Datetime must be on or after {minimum}",
    ),
    ("too_small.time", "Time must be at or after {minimum}"),
    ("too_small.duration", "Duration must be at least {minimum}s"),
    (
        "too_small.file",
        "File size must be at least {minimum} bytes",
    ),
    ("too_small.image_width", "Image width must be >= {minimum}"),
    (
        "too_small.image_height",
        "Image height must be >= {minimum}",
    ),
    ("too_small.port", "Port must be at least {minimum}"),
    (
        "too_small.contains",
        "Array must contain required value at least {minimum} time(s)",
    ),
    (
        "too_small_exclusive",
        "Value must be greater than {minimum}",
    ),
    (
        "too_small_exclusive.number",
        "Number must be greater than {minimum}",
    ),
    (
        "too_small_exclusive.decimal",
        "Decimal must be greater than {minimum}",
    ),
    // Upper bounds
    ("too_big", "Value must be at most {maximum}"),
    (
        "too_big.string",
        "String must be at most {maximum} characters",
    ),
    (
        "too_big.string_one",
        "String must be at most {maximum} character",
    ),
    ("too_big.number", "Number must be at most {maximum}"),
    ("too_big.decimal", "Decimal must be at most {maximum}"),
    (
        "too_big.array",
        "Array must have at most {maximum} elements",
    ),
    (
        "too_big.array_one",
        "Array must have at most {maximum} element",
    ),
    (
        "too_big.set",
        "Set must have at most {maximum} unique elements",
    ),
    (
        "too_big.set_one",
        "Set must have at most {maximum} unique element",
    ),
    ("too_big.record", "Record must have at most {maximum} keys"),
    (
        "too_big.record_one",
        "Record must have at most {maximum} key",
    ),
    (
        "too_big.object",
        "Object must contain at most {maximum} keys",
    ),
    (
        "too_big.object_one",
        "Object must contain at most {maximum} key",
    ),
    ("too_big.bytes", "Bytes length must be at most {maximum}"),
    ("too_big.date", "Date must be on or before {maximum}"),
    (
        "too_big.datetime",
        "Datetime must be on or before {maximum}",
    ),
    ("too_big.time", "Time must be at or before {maximum}"),
    ("too_big.duration", "Duration must be at most {maximum}s"),
    ("too_big.file", "File size must be at most {maximum} bytes"),
    ("too_big.image_width", "Image width must be <= {maximum}"),
    ("too_big.image_height", "Image height must be <= {maximum}"),
    ("too_big.port", "Port must be at most {maximum}"),
    (
        "too_big.contains",
        "Array must contain required value at most {maximum} time(s)",
    ),
    ("too_big_exclusive", "Value must be less than {maximum}"),
    (
        "too_big_exclusive.number",
        "Number must be less than {maximum}",
    ),
    (
        "too_big_exclusive.decimal",
        "Decimal must be less than {maximum}",
    ),
    // Exact length
    ("invalid_length", "Length must be exactly {expected}"),
    (
        "invalid_length.string",
        "String must be exactly {expected} characters",
    ),
    (
        "invalid_length.string_one",
        "String must be exactly {expected} character",
    ),
    (
        "invalid_length.array",
        "Array must have exactly {expected} elements",
    ),
    (
        "invalid_length.array_one",
        "Array must have exactly {expected} element",
    ),
    (
        "invalid_length.tuple",
        "Expected tuple of {expected} elements",
    ),
    (
        "invalid_length.tuple_one",
        "Expected tuple of {expected} element",
    ),
    (
        "invalid_length.bytes",
        "Bytes length must be exactly {expected}",
    ),
    // Strings
    ("invalid_string", "Invalid string ({validation})"),
    ("invalid_string.email", "Invalid email address"),
    ("invalid_string.url", "Invalid URL"),
    ("invalid_string.url_strict", "Invalid strict URL"),
    ("invalid_string.uri", "Invalid URI"),
    ("invalid_string.uuid", "Invalid UUID"),
    ("invalid_string.uuid_v1", "Invalid UUID v1"),
    ("invalid_string.uuid_v4", "Invalid UUID v4"),
    ("invalid_string.uuid_v7", "Invalid UUID v7"),
    ("invalid_string.ip", "Invalid IP address"),
    ("invalid_string.slug", "Invalid slug"),
    ("invalid_string.color", "Invalid color"),
    ("invalid_string.currency", "Invalid currency code"),
    ("invalid_string.country_code", "Invalid country code"),
    ("invalid_string.locale", "Invalid locale"),
    ("invalid_string.cron", "Invalid cron expression"),
    ("invalid_string.regex", "String does not match pattern"),
    (
        "invalid_string.starts_with",
        "String must start with \"{prefix}\"",
    ),
    (
        "invalid_string.ends_with",
        "String must end with \"{suffix}\"",
    ),
    (
        "invalid_string.contains",
        "String must contain \"{substring}\"",
    ),
    ("invalid_string.ipv4", "Invalid IPv4 address"),
    ("invalid_string.ipv6", "Invalid IPv6 address"),
    ("invalid_string.cidr", "Invalid CIDR"),
    ("invalid_string.mac", "Invalid MAC address"),
    ("invalid_string.hex", "Invalid hexadecimal string"),
    ("invalid_string.credit_card", "Invalid credit card number"),
    ("invalid_string.phone", "Invalid phone number"),
    ("invalid_string.phone_e164", "Invalid E.164 phone number"),
    ("invalid_string.semver", "Invalid semver"),
    ("invalid_string.semver_full", "Invalid semantic version"),
    ("invalid_string.jwt", "Invalid JWT"),
    (
        "invalid_string.ascii",
        "String must contain only ASCII characters",
    ),
    (
        "invalid_string.alpha",
        "String must contain only alphabetic characters",
    ),
    (
        "invalid_string.alphanumeric",
        "String must contain only alphanumeric characters",
    ),
    ("invalid_string.lowercase", "String must be lowercase"),
    ("invalid_string.uppercase", "String must be uppercase"),
    ("invalid_string.base64", "Invalid Base64 string"),
    ("invalid_string.base64url", "Invalid Base64URL string"),
    ("invalid_string.json", "Invalid JSON string"),
    (
        "invalid_string.iso_date",
        "Invalid ISO date (expected YYYY-MM-DD)",
    ),
    ("invalid_string.iso_datetime", "Invalid ISO datetime"),
    ("invalid_string.iso_time", "Invalid ISO time"),
    ("invalid_string.hostname", "Invalid hostname"),
    ("invalid_string.cuid2", "Invalid CUID2"),
    ("invalid_string.ulid", "Invalid ULID"),
    ("invalid_string.nanoid", "Invalid Nano ID"),
    ("invalid_string.emoji", "String must contain only emoji"),
    // Other formats
    ("invalid_format", "Invalid {format}"),
    ("invalid_format.decimal", "Invalid decimal value"),
    ("invalid_format.ip_network", "Invalid IP network/CIDR"),
    ("invalid_format.ipv4_network", "Expected IPv4 network"),
    ("invalid_format.ipv6_network", "Expected IPv6 network"),
    (
        "invalid_format.socket_addr",
        "Invalid socket address (expected `ipv4:port` or `[ipv6]:port`; hostnames are not resolved)",
    ),
    ("invalid_format.base64", "Invalid Base64 byte string"),
    ("invalid_format.base64url", "Invalid Base64URL byte string"),
    ("invalid_format.hex", "Invalid hex byte string"),
    ("invalid_format.image", "Unable to decode image"),
    ("invalid_format.from_str", "Invalid value"),
    // Dates
    ("invalid_date", "Invalid {kind}"),
    (
        "invalid_date.date",
        "Invalid date format: expected YYYY-MM-DD",
    ),
    ("invalid_date.datetime", "Invalid datetime format"),
    (
        "invalid_date.time",
        "Invalid time format: expected HH:MM:SS[.fraction]",
    ),
    (
        "invalid_timezone_offset",
        "Timezone offset must be {expected}",
    ),
    ("not_in_past", "Value must be in the past"),
    ("not_in_past.date", "Date must be in the past"),
    ("not_in_past.datetime", "Datetime must be in the past"),
    ("not_in_future", "Value must be in the future"),
    ("not_in_future.date", "Date must be in the future"),
    ("not_in_future.datetime", "Datetime must be in the future"),
    // Numbers
    ("not_int", "Expected integer"),
    ("not_finite", "Number must be finite"),
    ("not_multiple_of", "Number must be a multiple of {divisor}"),
    (
        "not_safe_integer",
        "Number must be a safe integer (-(2^53-1) to 2^53-1)",
    ),
    // Values, unions, collections, objects
    ("invalid_literal", "Expected literal {expected}"),
    (
        "invalid_enum_value",
        "Invalid enum value. Expected one of: {options}",
    ),
    (
        "invalid_union",
        "Input did not match any variant of the union",
    ),
    // Prefix of each variant's issues inside a union error.
    ("union_variant", "[variant {n}]"),
    (
        "invalid_discriminator",
        "Invalid discriminator value. Expected one of: {options}",
    ),
    ("not_unique", "Array items must be unique"),
    ("missing_contains", "Array must contain required value"),
    ("duplicate_key", "Duplicate map key"),
    (
        "invalid_map_entry",
        "Each Map entry must be a [key, value] array of length 2",
    ),
    ("missing_field", "Required field is missing"),
    ("unrecognized_field", "Unrecognized field"),
    // Paths and files
    (
        "path_outside_base",
        "Path must be within base directory: {base}",
    ),
    ("path_not_absolute", "Path must be absolute"),
    ("path_not_relative", "Path must be relative"),
    ("path_not_found", "Path does not exist"),
    ("path_not_file", "Path is not a file"),
    ("path_not_dir", "Path is not a directory"),
    ("json_too_deep", "JSON depth exceeds max {max_depth}"),
    (
        "file_type_not_allowed",
        "File {kind} `{actual}` is not allowed (expected one of: {allowed})",
    ),
    ("file_type_denied", "File {kind} `{actual}` is denied"),
    ("file_type_unknown", "Unable to detect file {kind}"),
    ("checksum_mismatch", "{algorithm} checksum mismatch"),
    ("missing_exif", "Image must contain EXIF metadata"),
    // Plumbing
    ("serialize_error", "Serialization error: {detail}"),
    ("io_error", "I/O error"),
    ("parse_error", "Failed to parse input"),
    ("custom", "Validation error: {code}"),
];

type TemplateMap =
    HashMap<&'static str, (Option<&'static str>, HashMap<&'static str, &'static str>)>;

/// [`ENGLISH`] indexed as `generic key -> (generic template, sub-key -> template)`,
/// so lookups need no key formatting.
fn english_map() -> &'static TemplateMap {
    static MAP: std::sync::OnceLock<TemplateMap> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: TemplateMap = HashMap::new();
        for (key, template) in ENGLISH {
            match key.split_once('.') {
                Some((base, sub)) => {
                    map.entry(base).or_default().1.insert(sub, template);
                }
                None => map.entry(key).or_default().0 = Some(template),
            }
        }
        map
    })
}

/// Default English template for exactly `base.sub` (no fallback).
pub(crate) fn english_template_exact(base: &str, sub: &str) -> Option<&'static str> {
    english_map().get(base)?.1.get(sub).copied()
}

/// Default English template for `base.sub`, falling back to `base`.
pub(crate) fn english_template_for(base: &str, sub: Option<&str>) -> Option<&'static str> {
    let (generic, subs) = english_map().get(base)?;
    sub.and_then(|s| subs.get(s).copied()).or(*generic)
}

/// Default English template for one exact key (`"too_small.string"`).
#[cfg(test)]
pub(crate) fn english_template(key: &str) -> Option<&'static str> {
    match key.split_once('.') {
        Some((base, sub)) => english_map().get(base)?.1.get(sub).copied(),
        None => english_map().get(key)?.0,
    }
}

/// Create a [`MapResolver`] with the English (default) messages.
///
/// These are exactly the templates `vld` renders its default messages from,
/// so it is a complete base for overriding specific keys.
pub fn english() -> MapResolver {
    MapResolver::new(
        ENGLISH
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

fn table(t: &[(&str, &str)]) -> MapResolver {
    MapResolver::new(
        t.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

/// Create a [`MapResolver`] with Russian messages (every English key, plus
/// `type.<name>` translations of the `{expected}` / `{received}` type names).
pub fn russian() -> MapResolver {
    table(super::i18n_langs::RUSSIAN).with_plural_rule(plural_ru)
}

/// Create a [`MapResolver`] with German messages (every English key, plus
/// `type.<name>` translations).
pub fn german() -> MapResolver {
    table(super::i18n_langs::GERMAN)
}

/// Create a [`MapResolver`] with Spanish messages (every English key, plus
/// `type.<name>` translations).
pub fn spanish() -> MapResolver {
    table(super::i18n_langs::SPANISH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s
            .split('{')
            .skip(1)
            .filter_map(|p| p.split_once('}').map(|(n, _)| n.to_string()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn every_language_covers_every_english_key() {
        for (name, t) in [
            ("russian", super::super::i18n_langs::RUSSIAN),
            ("german", super::super::i18n_langs::GERMAN),
            ("spanish", super::super::i18n_langs::SPANISH),
        ] {
            let keys: std::collections::HashSet<&str> = t.iter().map(|(k, _)| *k).collect();
            assert_eq!(keys.len(), t.len(), "{name}: duplicate key");
            for (key, en) in ENGLISH {
                let tr = t
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| *v)
                    .unwrap_or_else(|| panic!("{name}: missing `{key}`"));
                assert_eq!(placeholders(tr), placeholders(en), "{name}: `{key}`");
            }
        }
    }
}

#[cfg(test)]
mod live_round3 {
    use crate::prelude::*;

    #[test]
    fn integer_bound_of_one_keeps_number_wording() {
        let err = crate::u16()
            .min(1)
            .parse_value(&serde_json::json!(0))
            .unwrap_err();
        assert_eq!(err.issues[0].message, "Number must be at least 1");
        let err = crate::i64()
            .max(1)
            .parse_value(&serde_json::json!(5))
            .unwrap_err();
        assert_eq!(err.issues[0].message, "Number must be at most 1");
        // The `_one` form still applies where it exists.
        let err = crate::char()
            .parse_value(&serde_json::json!("ab"))
            .unwrap_err();
        assert!(
            err.issues[0].message.contains("1 character"),
            "{}",
            err.issues[0].message
        );
    }

    #[test]
    fn union_variant_prefix_is_translated() {
        let err = crate::union(crate::string(), crate::number())
            .parse_value(&serde_json::json!(true))
            .unwrap_err();
        let ru = super::translate_error(&err, &super::russian());
        assert!(
            ru.issues
                .iter()
                .any(|i| i.message.starts_with("[вариант 1] ")),
            "{:?}",
            ru.issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
        assert!(!ru.issues.iter().any(|i| i.message.contains("[variant")));
    }
}
