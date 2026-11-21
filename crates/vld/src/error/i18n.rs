//! Internationalization (i18n) of validation error messages.
//!
//! Every message is rendered from a template looked up by the keys of
//! [`IssueCode::message_keys()`] and filled from [`IssueCode::params()`].
//! The templates live in JSON catalogs (`locales/{locale}.json`, flat keys
//! `vld.<message key>` and `vld.type.<type>`), the same files a frontend
//! (Vue / React through `@i18n-micro`, Leptos through `i18n-next`, a Tauri
//! WebView) uses to translate an issue's `code` and `params`:
//!
//! - without features, the default English messages come from `en.json`
//!   (no dependency); [`english()`] is that catalog as a [`MapResolver`];
//! - feature `i18n`: every shipped language through
//!   [i18n-next](https://docs.rs/i18n-next) — [`translator`] for the embedded
//!   catalogs (fallback `pt-BR` → `pt` → `en`, CLDR plural rules), or
//!   [`Translator::new`] over an app's own catalog, where the app overrides any
//!   `vld.*` key or adds a language;
//! - [`MapResolver`], [`FnResolver`] or an own [`MessageResolver`] for any other
//!   source (Fluent, gettext, a database).
//!
//! # Example
//!
//! ```
//! use vld::prelude::*;
//! use vld::i18n::{MapResolver, translate_error};
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

    /// The template of `key` for an issue that carries the count `n` (the
    /// bound of a size check, [`IssueCode::plural_count`]).
    ///
    /// Default: `<key>_<category>` (category from
    /// [`plural_category`](Self::plural_category), skipped for `"other"`), then
    /// `<key>`. [`Translator`] picks the form of an i18n-next plural message
    /// (`"=0 … | one | other"`) by the CLDR rules of its locale instead.
    fn resolve_count(&self, key: &str, n: u64) -> Option<String> {
        let category = self.plural_category(n);
        (category != "other")
            .then(|| self.resolve(&format!("{key}_{category}")))
            .flatten()
            .or_else(|| self.resolve(key))
    }

    /// Plural category of `n` (`"one"`, `"few"`, `"many"`, `"other"`) for the
    /// default [`resolve_count`](Self::resolve_count). Default: English rule
    /// ([`plural_en`]).
    fn plural_category(&self, n: u64) -> &'static str {
        plural_en(n)
    }
}

/// English plural rule: `1` → `"one"`, else `"other"`.
pub fn plural_en(n: u64) -> &'static str {
    if n == 1 { "one" } else { "other" }
}

/// Simple [`MessageResolver`] backed by a `HashMap<String, String>`.
///
/// Count variants are separate keys (`too_small.string_one`, `…_few`), chosen
/// by the plural rule ([`with_plural_rule`](Self::with_plural_rule)).
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

    /// Use another plural rule (`n` → `"one"` / `"few"` / `"many"` / `"other"`).
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
    /// Wrap a translation function.
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
/// returned unchanged. A union's `[variant N] ` prefix is translated through
/// the `union_variant` key.
///
/// **Plurals**: for codes with a count ([`IssueCode::plural_count`]: the
/// `minimum` / `maximum` / `expected` of size checks) the template comes from
/// [`MessageResolver::resolve_count`].
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
    let count = issue.code.plural_count();
    let template = issue.code.message_keys().iter().find_map(|k| match count {
        Some(n) => resolver.resolve_count(k, n),
        None => resolver.resolve(k),
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
// The English catalog: the source of every default message
// ---------------------------------------------------------------------------

/// `locales/en.json`: the default messages, also shipped to frontends.
const EN_JSON: &str = include_str!("../../locales/en.json");

type TemplateMap = HashMap<String, (Option<String>, HashMap<String, String>)>;

/// `en.json` read once: flat templates (`key`, `key_one` for a plural message)
/// and the same indexed as `generic key -> (generic template, sub-key -> template)`.
struct English {
    flat: Vec<(String, String)>,
    map: TemplateMap,
}

/// `"=0 zero | one | other"` → `[("key", other), ("key_one", one)]`; a value
/// without `|` is its own template.
fn flatten_english(key: &str, value: &str, out: &mut Vec<(String, String)>) {
    let forms: Vec<&str> = value.split('|').map(str::trim).collect();
    match forms.as_slice() {
        [_, one, .., other] => {
            out.push((key.to_string(), (*other).to_string()));
            out.push((format!("{key}_one"), (*one).to_string()));
        }
        _ => out.push((key.to_string(), value.to_string())),
    }
}

fn english_data() -> &'static English {
    static DATA: std::sync::OnceLock<English> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EN_JSON).expect("locales/en.json is a JSON object");
        let mut flat = Vec::with_capacity(json.len() + 16);
        for (key, value) in &json {
            if let (Some(key), Some(value)) = (key.strip_prefix("vld."), value.as_str()) {
                flatten_english(key, value, &mut flat);
            }
        }
        let mut map: TemplateMap = HashMap::new();
        for (key, template) in &flat {
            match key.split_once('.') {
                Some((base, sub)) => {
                    map.entry(base.to_string())
                        .or_default()
                        .1
                        .insert(sub.to_string(), template.clone());
                }
                None => map.entry(key.clone()).or_default().0 = Some(template.clone()),
            }
        }
        English { flat, map }
    })
}

/// Default English template for exactly `base.sub` (no fallback).
pub(crate) fn english_template_exact(base: &str, sub: &str) -> Option<&'static str> {
    english_data().map.get(base)?.1.get(sub).map(String::as_str)
}

/// Default English template for `base.sub`, falling back to `base`.
pub(crate) fn english_template_for(base: &str, sub: Option<&str>) -> Option<&'static str> {
    let (generic, subs) = english_data().map.get(base)?;
    sub.and_then(|s| subs.get(s))
        .or(generic.as_ref())
        .map(String::as_str)
}

/// Default English template for one exact key (`"too_small.string"`).
#[cfg(test)]
pub(crate) fn english_template(key: &str) -> Option<&'static str> {
    match key.split_once('.') {
        Some((base, sub)) => english_template_exact(base, sub),
        None => english_data().map.get(key)?.0.as_deref(),
    }
}

/// A [`MapResolver`] with the English (default) messages of `en.json`.
///
/// These are exactly the templates `vld` renders its default messages from
/// (a plural message as `key` / `key_one`), so it is a complete base for
/// overriding specific keys.
pub fn english() -> MapResolver {
    MapResolver::new(english_data().flat.iter().cloned().collect())
}

// ---------------------------------------------------------------------------
// Embedded catalogs through i18n-next (feature `i18n`)
// ---------------------------------------------------------------------------

/// The i18n-next version vld builds on, for [`Translator::new`] and
/// [`files()`] without a second dependency line.
#[cfg(feature = "i18n")]
pub use i18n_next;

/// Every shipped catalog: `(locale, JSON)`.
#[cfg(feature = "i18n")]
const CATALOGS: &[(&str, &str)] = &[
    ("ar", include_str!("../../locales/ar.json")),
    ("bg", include_str!("../../locales/bg.json")),
    ("cs", include_str!("../../locales/cs.json")),
    ("da", include_str!("../../locales/da.json")),
    ("de", include_str!("../../locales/de.json")),
    ("el", include_str!("../../locales/el.json")),
    ("en", EN_JSON),
    ("es", include_str!("../../locales/es.json")),
    ("fi", include_str!("../../locales/fi.json")),
    ("fr", include_str!("../../locales/fr.json")),
    ("he", include_str!("../../locales/he.json")),
    ("hi", include_str!("../../locales/hi.json")),
    ("hu", include_str!("../../locales/hu.json")),
    ("id", include_str!("../../locales/id.json")),
    ("it", include_str!("../../locales/it.json")),
    ("ja", include_str!("../../locales/ja.json")),
    ("ko", include_str!("../../locales/ko.json")),
    ("nb", include_str!("../../locales/nb.json")),
    ("nl", include_str!("../../locales/nl.json")),
    ("pl", include_str!("../../locales/pl.json")),
    ("pt", include_str!("../../locales/pt.json")),
    ("ro", include_str!("../../locales/ro.json")),
    ("ru", include_str!("../../locales/ru.json")),
    ("sv", include_str!("../../locales/sv.json")),
    ("th", include_str!("../../locales/th.json")),
    ("tr", include_str!("../../locales/tr.json")),
    ("uk", include_str!("../../locales/uk.json")),
    ("vi", include_str!("../../locales/vi.json")),
    ("zh", include_str!("../../locales/zh.json")),
];

/// The locale codes of the embedded catalogs (`"ar"`, `"bg"`, …, `"zh"`).
#[cfg(feature = "i18n")]
pub fn locales() -> impl Iterator<Item = &'static str> {
    CATALOGS.iter().map(|(code, _)| *code)
}

/// The embedded JSON catalog of exactly `locale` (`"ru"`), to write next to a
/// frontend's own locale files or merge into another catalog.
#[cfg(feature = "i18n")]
pub fn catalog(locale: &str) -> Option<&'static str> {
    CATALOGS
        .iter()
        .find(|(code, _)| *code == locale)
        .map(|(_, json)| *json)
}

/// The embedded catalogs as files in the i18n-next layout (`("ru.json", bytes)`),
/// for an app's [`i18n_next::Builder::extend`]: vld's strings become a layer
/// under the app's own files, so the app overrides any `vld.*` key and adds a
/// language by providing `vld.*` keys in its `{locale}.json`.
#[cfg(feature = "i18n")]
pub fn files() -> impl Iterator<Item = (String, &'static [u8])> {
    CATALOGS
        .iter()
        .map(|(code, json)| (format!("{code}.json"), json.as_bytes()))
}

/// The embedded catalog for a language tag: the tag itself, then without its
/// last subtag (`pt-BR` → `pt`, `zh-Hans-CN` → `zh-Hans` → `zh`), ASCII case
/// and `_` / `-` aside; `no` reads `nb`, `iw` reads `he`. `None` when no
/// catalog matches.
#[cfg(feature = "i18n")]
pub fn supported_locale(tag: &str) -> Option<&'static str> {
    let tag = tag.replace('_', "-");
    let mut candidate = tag.as_str();
    loop {
        if let Some(code) = locales().find(|c| c.eq_ignore_ascii_case(candidate)) {
            return Some(code);
        }
        if candidate.eq_ignore_ascii_case("no") {
            return Some("nb");
        }
        if candidate.eq_ignore_ascii_case("iw") {
            return Some("he");
        }
        candidate = candidate.rsplit_once('-')?.0;
    }
}

/// The process-wide catalog of [`CATALOGS`], each locale parsed on first use.
#[cfg(feature = "i18n")]
fn embedded() -> &'static i18n_next::I18n {
    static I18N: std::sync::OnceLock<i18n_next::I18n> = std::sync::OnceLock::new();
    I18N.get_or_init(|| {
        i18n_next::I18n::builder()
            .locale("en")
            .fallback("en")
            .lazy_files(files())
            .build()
            .expect("the embedded vld catalogs build")
    })
}

/// A [`MessageResolver`] over i18n-next catalogs: `vld.<key>` lookups, plural
/// forms picked by the CLDR rules of the locale.
///
/// - [`translator(locale)`](translator): the embedded catalogs;
/// - [`Translator::new(view)`](Translator::new): an app's own catalog first,
///   then the embedded one.
///
/// ```
/// use vld::prelude::*;
/// use vld::i18n::{translate_error, translator};
///
/// let err = vld::string().len(3).parse(r#""ab""#).unwrap_err();
/// let ru = translate_error(&err, &translator("ru-RU"));
/// assert_eq!(ru.issues[0].message, "Строка должна содержать ровно 3 символа");
/// ```
#[cfg(feature = "i18n")]
#[derive(Clone)]
pub struct Translator {
    view: i18n_next::View,
    /// The embedded catalog in the app's locale, after the app's own ([`Translator::new`]).
    embedded: Option<i18n_next::View>,
}

/// A [`Translator`] for `locale` over the embedded catalogs: the catalog of
/// [`supported_locale`] (`pt-BR` → `pt`), then English for a missing key; an
/// unknown locale reads English.
#[cfg(feature = "i18n")]
pub fn translator(locale: &str) -> Translator {
    Translator {
        view: embedded().with_locale(supported_locale(locale).unwrap_or("en")),
        embedded: None,
    }
}

#[cfg(feature = "i18n")]
impl Translator {
    /// A translator over an app's i18n-next catalog (`i18n.with_locale("de")`,
    /// or `i18n.page("index")` to follow the active locale).
    ///
    /// `vld.*` keys are looked up in the app's view (its own chain: page, locale,
    /// fallbacks), then in the embedded catalog of the view's locale. Build the
    /// app's catalog with [`files()`] in
    /// [`Builder::extend`](i18n_next::Builder::extend) so that the order is the
    /// app's locale, vld's locale, then the fallbacks (an app override in the
    /// fallback language then never hides vld's own translation).
    pub fn new(app: i18n_next::View) -> Self {
        let embedded = translator(&app.locale()).view;
        Translator {
            view: app,
            embedded: Some(embedded),
        }
    }

    /// The view that has `vld.<key>`, and that key.
    fn find(&self, key: &str) -> Option<(&i18n_next::View, String)> {
        let key = format!("vld.{key}");
        if self.view.get(&key).is_some() {
            return Some((&self.view, key));
        }
        let embedded = self.embedded.as_ref()?;
        embedded.get(&key)?;
        Some((embedded, key))
    }
}

#[cfg(feature = "i18n")]
impl MessageResolver for Translator {
    fn resolve(&self, key: &str) -> Option<String> {
        let (view, key) = self.find(key)?;
        view.get(&key).map(str::to_string)
    }

    fn resolve_count(&self, key: &str, n: u64) -> Option<String> {
        let (view, key) = self.find(key)?;
        Some(view.tc(&key, n))
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

    #[cfg(feature = "i18n")]
    #[test]
    fn union_variant_prefix_is_translated() {
        let err = crate::union(crate::string(), crate::number())
            .parse_value(&serde_json::json!(true))
            .unwrap_err();
        let ru = super::translate_error(&err, &super::translator("ru"));
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
