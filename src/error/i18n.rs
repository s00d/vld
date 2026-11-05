//! Internationalization (i18n) support for validation error messages.
//!
//! Provides [`MessageResolver`] trait and built-in resolvers to translate
//! [`VldError`](crate::error::VldError) messages using [`IssueCode::key()`] and
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
}

impl MapResolver {
    pub fn new(map: HashMap<String, String>) -> Self {
        Self { map }
    }
}

impl MessageResolver for MapResolver {
    fn resolve(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
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

/// Apply parameter substitution to a template string.
///
/// Replaces `{param_name}` placeholders with values from `params`.
pub(crate) fn apply_params(template: &str, params: &[(&str, String)]) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}

/// Translate a single issue using the resolver.
///
/// The resolver is asked for every key of [`IssueCode::message_keys()`],
/// most specific first (e.g. `"too_small.string"`, then `"too_small"`); the
/// first template found is filled with [`IssueCode::params()`]. Without any
/// match the original message is kept.
///
/// Exclusive bounds (`gt()`, `lt()`, `positive()`, …) use the
/// `"too_small_exclusive"` / `"too_big_exclusive"` keys, so an inclusive
/// "at least" template is never applied to a "greater than" check.
pub fn translate_issue(issue: &ValidationIssue, resolver: &dyn MessageResolver) -> ValidationIssue {
    let message = issue
        .code
        .message_keys()
        .iter()
        .find_map(|k| resolver.resolve(k))
        .map(|template| apply_params(&template, &issue.code.params()))
        .unwrap_or_else(|| issue.message.clone());
    ValidationIssue {
        code: issue.code.clone(),
        message,
        path: issue.path.clone(),
        received: issue.received.clone(),
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
    // Lower bounds
    ("too_small", "Value must be at least {minimum}"),
    ("too_small.string", "String must be at least {minimum} characters"),
    ("too_small.number", "Number must be at least {minimum}"),
    ("too_small.decimal", "Decimal must be at least {minimum}"),
    ("too_small.array", "Array must have at least {minimum} elements"),
    ("too_small.set", "Set must have at least {minimum} unique elements"),
    ("too_small.record", "Record must have at least {minimum} keys"),
    ("too_small.object", "Object must contain at least {minimum} keys"),
    ("too_small.bytes", "Bytes length must be at least {minimum}"),
    ("too_small.date", "Date must be on or after {minimum}"),
    ("too_small.datetime", "Datetime must be on or after {minimum}"),
    ("too_small.duration", "Duration must be at least {minimum}s"),
    ("too_small.file", "File size must be at least {minimum} bytes"),
    ("too_small.image_width", "Image width must be >= {minimum}"),
    ("too_small.image_height", "Image height must be >= {minimum}"),
    ("too_small.port", "Port must be at least {minimum}"),
    (
        "too_small.contains",
        "Array must contain required value at least {minimum} time(s)",
    ),
    ("too_small_exclusive", "Value must be greater than {minimum}"),
    ("too_small_exclusive.number", "Number must be greater than {minimum}"),
    ("too_small_exclusive.decimal", "Decimal must be greater than {minimum}"),
    // Upper bounds
    ("too_big", "Value must be at most {maximum}"),
    ("too_big.string", "String must be at most {maximum} characters"),
    ("too_big.number", "Number must be at most {maximum}"),
    ("too_big.decimal", "Decimal must be at most {maximum}"),
    ("too_big.array", "Array must have at most {maximum} elements"),
    ("too_big.set", "Set must have at most {maximum} unique elements"),
    ("too_big.record", "Record must have at most {maximum} keys"),
    ("too_big.object", "Object must contain at most {maximum} keys"),
    ("too_big.bytes", "Bytes length must be at most {maximum}"),
    ("too_big.date", "Date must be on or before {maximum}"),
    ("too_big.datetime", "Datetime must be on or before {maximum}"),
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
    ("too_big_exclusive.number", "Number must be less than {maximum}"),
    ("too_big_exclusive.decimal", "Decimal must be less than {maximum}"),
    // Exact length
    ("invalid_length", "Length must be exactly {expected}"),
    ("invalid_length.string", "String must be exactly {expected} characters"),
    ("invalid_length.array", "Array must have exactly {expected} elements"),
    ("invalid_length.tuple", "Expected tuple of {expected} elements"),
    ("invalid_length.bytes", "Bytes length must be exactly {expected}"),
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
    ("invalid_string.starts_with", "String must start with \"{prefix}\""),
    ("invalid_string.ends_with", "String must end with \"{suffix}\""),
    ("invalid_string.contains", "String must contain \"{substring}\""),
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
    // Dates
    ("invalid_date", "Invalid {kind}"),
    (
        "invalid_date.date",
        "Invalid date format: expected YYYY-MM-DD",
    ),
    ("invalid_date.datetime", "Invalid datetime format"),
    ("invalid_timezone_offset", "Timezone offset must be {expected}"),
    ("not_in_past", "Value must be in the past"),
    ("not_in_past.date", "Date must be in the past"),
    ("not_in_past.datetime", "Datetime must be in the past"),
    ("not_in_future", "Value must be in the future"),
    ("not_in_future.date", "Date must be in the future"),
    ("not_in_future.datetime", "Datetime must be in the future"),
    // Numbers
    ("not_int", "Expected integer, received float"),
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
    ("invalid_union", "Input did not match any variant of the union"),
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
    ("path_outside_base", "Path must be within base directory: {base}"),
    ("path_not_absolute", "Path must be absolute"),
    ("path_not_relative", "Path must be relative"),
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

/// Default English template for one key (see [`english()`]).
pub(crate) fn english_template(key: &str) -> Option<&'static str> {
    static MAP: std::sync::OnceLock<HashMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| ENGLISH.iter().copied().collect())
        .get(key)
        .copied()
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

/// Create a [`MapResolver`] with Russian messages.
pub fn russian() -> MapResolver {
    let mut m = HashMap::new();
    m.insert(
        "invalid_type".into(),
        "Ожидалось {expected}, получено {received}".into(),
    );
    m.insert(
        "too_small".into(),
        "Значение должно быть не менее {minimum}".into(),
    );
    m.insert(
        "too_big".into(),
        "Значение должно быть не более {maximum}".into(),
    );
    m.insert(
        "too_small_exclusive".into(),
        "Значение должно быть больше {minimum}".into(),
    );
    m.insert(
        "too_big_exclusive".into(),
        "Значение должно быть меньше {maximum}".into(),
    );
    m.insert(
        "invalid_string".into(),
        "Некорректное значение ({validation})".into(),
    );
    m.insert("not_int".into(), "Ожидалось целое число".into());
    m.insert("not_finite".into(), "Число должно быть конечным".into());
    m.insert(
        "missing_field".into(),
        "Обязательное поле отсутствует".into(),
    );
    m.insert("unrecognized_field".into(), "Неизвестное поле".into());
    m.insert("parse_error".into(), "Ошибка разбора входных данных".into());
    MapResolver::new(m)
}

/// Create a [`MapResolver`] with German messages.
pub fn german() -> MapResolver {
    let mut m = HashMap::new();
    m.insert(
        "invalid_type".into(),
        "{expected} erwartet, {received} erhalten".into(),
    );
    m.insert(
        "too_small".into(),
        "Wert muss mindestens {minimum} sein".into(),
    );
    m.insert(
        "too_big".into(),
        "Wert darf höchstens {maximum} sein".into(),
    );
    m.insert(
        "too_small_exclusive".into(),
        "Wert muss größer als {minimum} sein".into(),
    );
    m.insert(
        "too_big_exclusive".into(),
        "Wert muss kleiner als {maximum} sein".into(),
    );
    m.insert(
        "invalid_string".into(),
        "Ungültiger Wert ({validation})".into(),
    );
    m.insert("not_int".into(), "Ganzzahl erwartet".into());
    m.insert("not_finite".into(), "Zahl muss endlich sein".into());
    m.insert("missing_field".into(), "Pflichtfeld fehlt".into());
    m.insert("unrecognized_field".into(), "Unbekanntes Feld".into());
    m.insert(
        "parse_error".into(),
        "Eingabe konnte nicht verarbeitet werden".into(),
    );
    MapResolver::new(m)
}

/// Create a [`MapResolver`] with Spanish messages.
pub fn spanish() -> MapResolver {
    let mut m = HashMap::new();
    m.insert(
        "invalid_type".into(),
        "Se esperaba {expected}, se recibió {received}".into(),
    );
    m.insert(
        "too_small".into(),
        "El valor debe ser al menos {minimum}".into(),
    );
    m.insert(
        "too_big".into(),
        "El valor debe ser como máximo {maximum}".into(),
    );
    m.insert(
        "too_small_exclusive".into(),
        "El valor debe ser mayor que {minimum}".into(),
    );
    m.insert(
        "too_big_exclusive".into(),
        "El valor debe ser menor que {maximum}".into(),
    );
    m.insert(
        "invalid_string".into(),
        "Valor inválido ({validation})".into(),
    );
    m.insert("not_int".into(), "Se esperaba un número entero".into());
    m.insert("not_finite".into(), "El número debe ser finito".into());
    m.insert("missing_field".into(), "Campo obligatorio faltante".into());
    m.insert("unrecognized_field".into(), "Campo no reconocido".into());
    m.insert("parse_error".into(), "Error al procesar la entrada".into());
    MapResolver::new(m)
}
