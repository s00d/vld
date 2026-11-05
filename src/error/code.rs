//! [`IssueCode`] and the small enums it carries.

use serde_json::Value;
use std::borrow::Cow;

/// What a size / range bound (`TooSmall`, `TooBig`, `InvalidLength`) was
/// measured on. Selects the message template (`too_small.<origin>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SizeOrigin {
    /// String length in characters.
    String,
    /// Numeric value (`ZNumber`, `ZInt`, byte values).
    Number,
    /// Arbitrary-precision decimal value (bound is a string).
    Decimal,
    /// Array length.
    Array,
    /// Tuple length.
    Tuple,
    /// Number of unique set elements.
    Set,
    /// Number of record keys.
    Record,
    /// Number of object keys.
    Object,
    /// Decoded byte length.
    Bytes,
    /// Calendar date (bound is a `YYYY-MM-DD` string).
    Date,
    /// Datetime (bound is an RFC 3339 string).
    #[serde(rename = "datetime")]
    DateTime,
    /// Duration in seconds.
    Duration,
    /// File size in bytes.
    File,
    /// Image width in pixels.
    ImageWidth,
    /// Image height in pixels.
    ImageHeight,
    /// Socket address port.
    Port,
    /// Number of array items equal to the `contains(...)` value.
    Contains,
}

impl SizeOrigin {
    /// Stable snake_case key (`"string"`, `"datetime"`, `"image_width"`, …).
    pub fn key(&self) -> &'static str {
        match self {
            SizeOrigin::String => "string",
            SizeOrigin::Number => "number",
            SizeOrigin::Decimal => "decimal",
            SizeOrigin::Array => "array",
            SizeOrigin::Tuple => "tuple",
            SizeOrigin::Set => "set",
            SizeOrigin::Record => "record",
            SizeOrigin::Object => "object",
            SizeOrigin::Bytes => "bytes",
            SizeOrigin::Date => "date",
            SizeOrigin::DateTime => "datetime",
            SizeOrigin::Duration => "duration",
            SizeOrigin::File => "file",
            SizeOrigin::ImageWidth => "image_width",
            SizeOrigin::ImageHeight => "image_height",
            SizeOrigin::Port => "port",
            SizeOrigin::Contains => "contains",
        }
    }
}

/// Date or datetime, for the date-specific issue codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateKind {
    Date,
    #[serde(rename = "datetime")]
    DateTime,
}

impl DateKind {
    /// `"date"` or `"datetime"`.
    pub fn key(&self) -> &'static str {
        match self {
            DateKind::Date => "date",
            DateKind::DateTime => "datetime",
        }
    }
}

/// Non-string-schema value formats reported by [`IssueCode::InvalidFormat`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FormatKind {
    Decimal,
    IpNetwork,
    Ipv4Network,
    Ipv6Network,
    SocketAddr,
    Base64,
    Base64Url,
    Hex,
    Image,
    /// A `FromStr` parse failed ([`vld::from_str()`](crate::from_str)).
    FromStr,
}

impl FormatKind {
    /// Stable snake_case key.
    pub fn key(&self) -> &'static str {
        match self {
            FormatKind::Decimal => "decimal",
            FormatKind::IpNetwork => "ip_network",
            FormatKind::Ipv4Network => "ipv4_network",
            FormatKind::Ipv6Network => "ipv6_network",
            FormatKind::SocketAddr => "socket_addr",
            FormatKind::Base64 => "base64",
            FormatKind::Base64Url => "base64url",
            FormatKind::Hex => "hex",
            FormatKind::Image => "image",
            FormatKind::FromStr => "from_str",
        }
    }
}

/// Which file type property a file-type issue refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileTypeKind {
    Extension,
    MediaType,
    MagicType,
}

impl FileTypeKind {
    /// Stable snake_case key.
    pub fn key(&self) -> &'static str {
        match self {
            FileTypeKind::Extension => "extension",
            FileTypeKind::MediaType => "media_type",
            FileTypeKind::MagicType => "magic_type",
        }
    }

    /// Human-readable label used in messages (`"media type"`).
    pub fn label(&self) -> &'static str {
        match self {
            FileTypeKind::Extension => "extension",
            FileTypeKind::MediaType => "media type",
            FileTypeKind::MagicType => "magic type",
        }
    }
}

/// Type of string validation that failed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum StringValidation {
    Email,
    Url,
    UrlStrict,
    Uri,
    Uuid,
    UuidV1,
    UuidV4,
    UuidV7,
    Ip,
    Slug,
    Color,
    Currency,
    CountryCode,
    Locale,
    Cron,
    Regex,
    StartsWith { prefix: String },
    EndsWith { suffix: String },
    Contains { substring: String },
    Ipv4,
    Ipv6,
    Cidr,
    Mac,
    Hex,
    CreditCard,
    Phone,
    PhoneE164,
    Semver,
    SemverFull,
    Jwt,
    Ascii,
    Alpha,
    Alphanumeric,
    Lowercase,
    Uppercase,
    Base64,
    IsoDate,
    IsoDatetime,
    IsoTime,
    Hostname,
    Cuid2,
    Ulid,
    Nanoid,
    Emoji,
}

impl StringValidation {
    /// Stable snake_case key (`invalid_string.<key>` message template).
    pub fn key(&self) -> &'static str {
        use StringValidation::*;
        match self {
            Email => "email",
            Url => "url",
            UrlStrict => "url_strict",
            Uri => "uri",
            Uuid => "uuid",
            UuidV1 => "uuid_v1",
            UuidV4 => "uuid_v4",
            UuidV7 => "uuid_v7",
            Ip => "ip",
            Slug => "slug",
            Color => "color",
            Currency => "currency",
            CountryCode => "country_code",
            Locale => "locale",
            Cron => "cron",
            Regex => "regex",
            StartsWith { .. } => "starts_with",
            EndsWith { .. } => "ends_with",
            Contains { .. } => "contains",
            Ipv4 => "ipv4",
            Ipv6 => "ipv6",
            Cidr => "cidr",
            Mac => "mac",
            Hex => "hex",
            CreditCard => "credit_card",
            Phone => "phone",
            PhoneE164 => "phone_e164",
            Semver => "semver",
            SemverFull => "semver_full",
            Jwt => "jwt",
            Ascii => "ascii",
            Alpha => "alpha",
            Alphanumeric => "alphanumeric",
            Lowercase => "lowercase",
            Uppercase => "uppercase",
            Base64 => "base64",
            IsoDate => "iso_date",
            IsoDatetime => "iso_datetime",
            IsoTime => "iso_time",
            Hostname => "hostname",
            Cuid2 => "cuid2",
            Ulid => "ulid",
            Nanoid => "nanoid",
            Emoji => "emoji",
        }
    }
}

/// Validation issue code — describes what went wrong.
///
/// Every variant except [`Custom`](IssueCode::Custom) is produced by `vld`
/// itself; `Custom` is reserved for user-defined codes (`refine`, `custom`,
/// `super_refine`, `IssueBuilder`). See the [module docs](crate::error) for
/// the key of every variant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum IssueCode {
    /// Wrong JSON type.
    InvalidType { expected: String, received: String },
    /// Below a lower bound. `minimum` is a JSON number, or a string for
    /// `Decimal` / `Date` / `DateTime` origins.
    TooSmall {
        origin: SizeOrigin,
        minimum: Value,
        inclusive: bool,
    },
    /// Above an upper bound (see `TooSmall`).
    TooBig {
        origin: SizeOrigin,
        maximum: Value,
        inclusive: bool,
    },
    /// Exact length mismatch.
    InvalidLength { origin: SizeOrigin, expected: usize },
    /// String format / content check failed.
    InvalidString { validation: StringValidation },
    /// A non-string schema could not parse its textual format.
    InvalidFormat { format: FormatKind },
    /// Not a valid date / datetime.
    InvalidDate { kind: DateKind },
    /// Datetime offset differs from the required one (`±HH:MM`).
    InvalidTimezoneOffset { expected: String },
    /// Date / datetime is not in the past.
    NotInPast { kind: DateKind },
    /// Date / datetime is not in the future.
    NotInFuture { kind: DateKind },
    /// Number is not an integer.
    NotInt,
    /// Number is NaN or infinite.
    NotFinite,
    /// Number is not a multiple of `divisor`.
    NotMultipleOf { divisor: Value },
    /// Number is outside `-(2^53-1)..=2^53-1`.
    NotSafeInteger,
    /// Value differs from the literal.
    InvalidLiteral { expected: Value },
    /// String is not one of the enum options.
    InvalidEnumValue { options: Vec<String> },
    /// No union variant matched (followed by every variant's issues).
    InvalidUnion,
    /// Discriminator value matches no variant.
    InvalidDiscriminator { options: Vec<Value> },
    /// Array item repeats an earlier one (`unique()`).
    NotUnique,
    /// Array lacks the `contains(...)` value.
    MissingContains,
    /// Map key repeats an earlier one.
    DuplicateKey,
    /// Map entry is not a `[key, value]` pair.
    InvalidMapEntry,
    /// Required field is missing or null.
    MissingField,
    /// Field not declared by a strict object.
    UnrecognizedField,
    /// Path escapes the `within(base)` directory.
    PathOutsideBase { base: String },
    /// Path must be absolute.
    PathNotAbsolute,
    /// Path must be relative.
    PathNotRelative,
    /// JSON nesting is deeper than `max_depth`.
    JsonTooDeep { max_depth: usize },
    /// File extension / media type / magic type is not in the allow list.
    FileTypeNotAllowed {
        kind: FileTypeKind,
        actual: String,
        allowed: Vec<String>,
    },
    /// File extension / media type / magic type is on the deny list.
    FileTypeDenied { kind: FileTypeKind, actual: String },
    /// File media / magic type could not be detected.
    FileTypeUnknown { kind: FileTypeKind },
    /// File hash differs (`algorithm` is `"SHA-256"` or `"MD5"`).
    ChecksumMismatch { algorithm: String },
    /// Image has no EXIF metadata.
    MissingExif,
    /// A validated value could not be serialized to JSON.
    SerializeError { detail: String },
    /// Filesystem error.
    IoError,
    /// Input is not valid JSON.
    ParseError,
    /// User-defined code.
    Custom { code: String },
}

/// Render a bound / literal for messages: strings without quotes, integral
/// floats without a fraction (`5`, not `5.0`).
pub(crate) fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 1e15 => (f as i64).to_string(),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn join<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    items.iter().map(f).collect::<Vec<_>>().join(", ")
}

impl IssueCode {
    /// Stable snake_case key for this code (the user code for `Custom`).
    pub fn key(&self) -> &str {
        use IssueCode::*;
        match self {
            InvalidType { .. } => "invalid_type",
            TooSmall { .. } => "too_small",
            TooBig { .. } => "too_big",
            InvalidLength { .. } => "invalid_length",
            InvalidString { .. } => "invalid_string",
            InvalidFormat { .. } => "invalid_format",
            InvalidDate { .. } => "invalid_date",
            InvalidTimezoneOffset { .. } => "invalid_timezone_offset",
            NotInPast { .. } => "not_in_past",
            NotInFuture { .. } => "not_in_future",
            NotInt => "not_int",
            NotFinite => "not_finite",
            NotMultipleOf { .. } => "not_multiple_of",
            NotSafeInteger => "not_safe_integer",
            InvalidLiteral { .. } => "invalid_literal",
            InvalidEnumValue { .. } => "invalid_enum_value",
            InvalidUnion => "invalid_union",
            InvalidDiscriminator { .. } => "invalid_discriminator",
            NotUnique => "not_unique",
            MissingContains => "missing_contains",
            DuplicateKey => "duplicate_key",
            InvalidMapEntry => "invalid_map_entry",
            MissingField => "missing_field",
            UnrecognizedField => "unrecognized_field",
            PathOutsideBase { .. } => "path_outside_base",
            PathNotAbsolute => "path_not_absolute",
            PathNotRelative => "path_not_relative",
            JsonTooDeep { .. } => "json_too_deep",
            FileTypeNotAllowed { .. } => "file_type_not_allowed",
            FileTypeDenied { .. } => "file_type_denied",
            FileTypeUnknown { .. } => "file_type_unknown",
            ChecksumMismatch { .. } => "checksum_mismatch",
            MissingExif => "missing_exif",
            SerializeError { .. } => "serialize_error",
            IoError => "io_error",
            ParseError => "parse_error",
            Custom { code } => code,
        }
    }

    /// Template key parts: the generic key and, when the code has one, the
    /// specific sub-key (`("too_small", Some("string"))`).
    fn template_key(&self) -> (&str, Option<&'static str>) {
        use IssueCode::*;
        match self {
            TooSmall {
                origin, inclusive, ..
            } => (
                if *inclusive {
                    "too_small"
                } else {
                    "too_small_exclusive"
                },
                Some(origin.key()),
            ),
            TooBig {
                origin, inclusive, ..
            } => (
                if *inclusive {
                    "too_big"
                } else {
                    "too_big_exclusive"
                },
                Some(origin.key()),
            ),
            InvalidLength { origin, .. } => ("invalid_length", Some(origin.key())),
            InvalidString { validation } => ("invalid_string", Some(validation.key())),
            InvalidFormat { format } => ("invalid_format", Some(format.key())),
            InvalidDate { kind } => ("invalid_date", Some(kind.key())),
            NotInPast { kind } => ("not_in_past", Some(kind.key())),
            NotInFuture { kind } => ("not_in_future", Some(kind.key())),
            other => (other.key(), None),
        }
    }

    /// Message-template keys, most specific first.
    ///
    /// E.g. `["too_small.string", "too_small"]`; exclusive bounds use
    /// `too_small_exclusive` / `too_big_exclusive`; `Custom` codes fall back
    /// to `"custom"`. Resolvers ([`MessageResolver`](super::i18n::MessageResolver))
    /// are asked in this order.
    pub fn message_keys(&self) -> Vec<String> {
        let (base, sub) = self.template_key();
        let mut keys = Vec::with_capacity(2);
        if let Some(sub) = sub {
            keys.push(format!("{}.{}", base, sub));
        }
        keys.push(base.to_string());
        if let IssueCode::Custom { .. } = self {
            keys.push("custom".to_string());
        }
        keys
    }

    /// One template parameter by name (`"minimum"`, `"expected"`, …).
    fn param(&self, name: &str) -> Option<Cow<'_, str>> {
        use IssueCode::*;
        let b = |v: &'static str| Some(Cow::Borrowed(v));
        match (self, name) {
            (InvalidType { expected, .. }, "expected") => Some(Cow::Borrowed(expected)),
            (InvalidType { received, .. }, "received") => Some(Cow::Borrowed(received)),
            (TooSmall { origin, .. }, "origin")
            | (TooBig { origin, .. }, "origin")
            | (InvalidLength { origin, .. }, "origin") => b(origin.key()),
            (TooSmall { minimum, .. }, "minimum") => Some(Cow::Owned(display_value(minimum))),
            (TooBig { maximum, .. }, "maximum") => Some(Cow::Owned(display_value(maximum))),
            (TooSmall { inclusive, .. }, "inclusive") | (TooBig { inclusive, .. }, "inclusive") => {
                b(if *inclusive { "true" } else { "false" })
            }
            (InvalidLength { expected, .. }, "expected") => Some(Cow::Owned(expected.to_string())),
            (InvalidString { validation }, "validation") => b(validation.key()),
            (InvalidString { validation }, _) => match (validation, name) {
                (StringValidation::StartsWith { prefix }, "prefix") => Some(Cow::Borrowed(prefix)),
                (StringValidation::EndsWith { suffix }, "suffix") => Some(Cow::Borrowed(suffix)),
                (StringValidation::Contains { substring }, "substring") => {
                    Some(Cow::Borrowed(substring))
                }
                _ => None,
            },
            (InvalidFormat { format }, "format") => b(format.key()),
            (InvalidDate { kind }, "kind")
            | (NotInPast { kind }, "kind")
            | (NotInFuture { kind }, "kind") => b(kind.key()),
            (InvalidTimezoneOffset { expected }, "expected") => Some(Cow::Borrowed(expected)),
            (NotMultipleOf { divisor }, "divisor") => Some(Cow::Owned(display_value(divisor))),
            (InvalidLiteral { expected }, "expected") => Some(Cow::Owned(expected.to_string())),
            (InvalidEnumValue { options }, "options") => {
                Some(Cow::Owned(join(options, |o| format!("\"{}\"", o))))
            }
            (InvalidDiscriminator { options }, "options") => {
                Some(Cow::Owned(join(options, |o| o.to_string())))
            }
            (PathOutsideBase { base }, "base") => Some(Cow::Borrowed(base)),
            (JsonTooDeep { max_depth }, "max_depth") => Some(Cow::Owned(max_depth.to_string())),
            (FileTypeNotAllowed { kind, .. }, "kind")
            | (FileTypeDenied { kind, .. }, "kind")
            | (FileTypeUnknown { kind }, "kind") => b(kind.label()),
            (FileTypeNotAllowed { actual, .. }, "actual")
            | (FileTypeDenied { actual, .. }, "actual") => Some(Cow::Borrowed(actual)),
            (FileTypeNotAllowed { allowed, .. }, "allowed") => Some(Cow::Owned(allowed.join(", "))),
            (ChecksumMismatch { algorithm }, "algorithm") => Some(Cow::Borrowed(algorithm)),
            (SerializeError { detail }, "detail") => Some(Cow::Borrowed(detail)),
            (Custom { code }, "code") => Some(Cow::Borrowed(code)),
            _ => None,
        }
    }

    /// Key-value parameters for message templates (`{minimum}`, `{expected}`, …).
    pub fn params(&self) -> Vec<(&'static str, String)> {
        PARAM_NAMES
            .iter()
            .filter_map(|name| self.param(name).map(|v| (*name, v.into_owned())))
            .collect()
    }

    /// Fill the `{name}` placeholders of `template` from this code's
    /// parameters (unknown placeholders are kept as is).
    pub(crate) fn render(&self, template: &str) -> String {
        let mut out = String::with_capacity(template.len() + 16);
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else {
                out.push_str(&rest[open..]);
                return out;
            };
            let name = &after[..close];
            match self.param(name) {
                Some(v) => out.push_str(&v),
                None => out.push_str(&rest[open..open + close + 2]),
            }
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Default English message, rendered from the
    /// [`english()`](super::i18n::english) template table and [`params()`](Self::params).
    pub fn default_message(&self) -> String {
        let (base, sub) = match self {
            IssueCode::Custom { .. } => ("custom", None),
            other => other.template_key(),
        };
        let template = super::i18n::english_template_for(base, sub).unwrap_or("{code}");
        self.render(template)
    }
}

/// Every parameter name a built-in code can provide (order of [`IssueCode::params`]).
const PARAM_NAMES: &[&str] = &[
    "expected",
    "received",
    "origin",
    "minimum",
    "maximum",
    "inclusive",
    "validation",
    "prefix",
    "suffix",
    "substring",
    "format",
    "kind",
    "divisor",
    "options",
    "base",
    "max_depth",
    "actual",
    "allowed",
    "algorithm",
    "detail",
    "code",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One value of every variant; the exhaustive `match` breaks the build
    /// when a variant is added without a sample here.
    fn samples() -> Vec<IssueCode> {
        use IssueCode::*;
        let all = vec![
            InvalidType {
                expected: "string".into(),
                received: "number".into(),
            },
            TooSmall {
                origin: SizeOrigin::String,
                minimum: json!(1),
                inclusive: true,
            },
            TooBig {
                origin: SizeOrigin::Number,
                maximum: json!(1),
                inclusive: false,
            },
            InvalidLength {
                origin: SizeOrigin::Array,
                expected: 2,
            },
            InvalidString {
                validation: StringValidation::StartsWith { prefix: "a".into() },
            },
            InvalidFormat {
                format: FormatKind::Hex,
            },
            InvalidDate {
                kind: DateKind::Date,
            },
            InvalidTimezoneOffset {
                expected: "+03:00".into(),
            },
            NotInPast {
                kind: DateKind::Date,
            },
            NotInFuture {
                kind: DateKind::DateTime,
            },
            NotInt,
            NotFinite,
            NotMultipleOf { divisor: json!(2) },
            NotSafeInteger,
            InvalidLiteral { expected: json!(1) },
            InvalidEnumValue {
                options: vec!["a".into()],
            },
            InvalidUnion,
            InvalidDiscriminator {
                options: vec![json!("a")],
            },
            NotUnique,
            MissingContains,
            DuplicateKey,
            InvalidMapEntry,
            MissingField,
            UnrecognizedField,
            PathOutsideBase { base: "/b".into() },
            PathNotAbsolute,
            PathNotRelative,
            JsonTooDeep { max_depth: 1 },
            FileTypeNotAllowed {
                kind: FileTypeKind::Extension,
                actual: "exe".into(),
                allowed: vec!["png".into()],
            },
            FileTypeDenied {
                kind: FileTypeKind::MediaType,
                actual: "x".into(),
            },
            FileTypeUnknown {
                kind: FileTypeKind::MagicType,
            },
            ChecksumMismatch {
                algorithm: "MD5".into(),
            },
            MissingExif,
            SerializeError { detail: "x".into() },
            IoError,
            ParseError,
            Custom {
                code: "mine".into(),
            },
        ];
        for c in &all {
            match c {
                InvalidType { .. }
                | TooSmall { .. }
                | TooBig { .. }
                | InvalidLength { .. }
                | InvalidString { .. }
                | InvalidFormat { .. }
                | InvalidDate { .. }
                | InvalidTimezoneOffset { .. }
                | NotInPast { .. }
                | NotInFuture { .. }
                | NotInt
                | NotFinite
                | NotMultipleOf { .. }
                | NotSafeInteger
                | InvalidLiteral { .. }
                | InvalidEnumValue { .. }
                | InvalidUnion
                | InvalidDiscriminator { .. }
                | NotUnique
                | MissingContains
                | DuplicateKey
                | InvalidMapEntry
                | MissingField
                | UnrecognizedField
                | PathOutsideBase { .. }
                | PathNotAbsolute
                | PathNotRelative
                | JsonTooDeep { .. }
                | FileTypeNotAllowed { .. }
                | FileTypeDenied { .. }
                | FileTypeUnknown { .. }
                | ChecksumMismatch { .. }
                | MissingExif
                | SerializeError { .. }
                | IoError
                | ParseError
                | Custom { .. } => {}
            }
        }
        all
    }

    #[test]
    fn every_code_has_an_english_template() {
        for code in samples() {
            let generic = code.message_keys().pop().unwrap();
            assert!(
                crate::error::i18n::english_template(&generic).is_some(),
                "missing English template for `{}`",
                generic
            );
        }
        for key in ["too_small_exclusive", "too_big_exclusive"] {
            assert!(crate::error::i18n::english_template(key).is_some());
        }
    }

    #[test]
    fn templates_only_use_known_params() {
        for code in samples() {
            for key in code.message_keys() {
                if let Some(t) = crate::error::i18n::english_template(&key) {
                    let filled = code.render(t);
                    assert!(!filled.contains('{'), "{}: {}", key, filled);
                }
            }
        }
    }

    #[test]
    fn every_size_origin_has_templates() {
        use SizeOrigin::*;
        for o in [
            String,
            Number,
            Decimal,
            Array,
            Set,
            Record,
            Object,
            Bytes,
            Date,
            DateTime,
            Duration,
            File,
            ImageWidth,
            ImageHeight,
            Port,
            Contains,
        ] {
            for base in ["too_small", "too_big"] {
                let key = format!("{}.{}", base, o.key());
                assert!(
                    crate::error::i18n::english_template(&key).is_some(),
                    "{key}"
                );
            }
        }
    }
}
