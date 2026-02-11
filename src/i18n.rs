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

use crate::error::{ValidationIssue, VldError};
use std::collections::HashMap;

/// Trait for resolving validation messages by error code key.
///
/// Implementations receive the stable string key from [`IssueCode::key()`]
/// and should return the translated template string, or `None` to keep the
/// original message.
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
fn apply_params(template: &str, params: &[(&str, String)]) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}

/// Translate a single issue using the resolver.
///
/// If the resolver returns a template for the issue's code key, the message
/// is replaced with the interpolated template. Otherwise the original message
/// is kept.
pub fn translate_issue(issue: &ValidationIssue, resolver: &dyn MessageResolver) -> ValidationIssue {
    let params = issue.code.params();
    let message = match resolver.resolve(issue.code.key()) {
        Some(template) => apply_params(&template, &params),
        None => issue.message.clone(),
    };
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

/// Create a [`MapResolver`] with English (default) messages.
///
/// These match the messages `vld` generates by default — useful as a base
/// for overriding specific keys.
pub fn english() -> MapResolver {
    let mut m = HashMap::new();
    m.insert(
        "invalid_type".into(),
        "Expected {expected}, received {received}".into(),
    );
    m.insert(
        "too_small".into(),
        "Value must be at least {minimum}".into(),
    );
    m.insert("too_big".into(), "Value must be at most {maximum}".into());
    m.insert("invalid_string".into(), "Invalid {validation}".into());
    m.insert("not_int".into(), "Expected integer, received float".into());
    m.insert("not_finite".into(), "Number must be finite".into());
    m.insert("missing_field".into(), "Required field is missing".into());
    m.insert("unrecognized_field".into(), "Unrecognized field".into());
    m.insert("parse_error".into(), "Failed to parse input".into());
    MapResolver::new(m)
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
