use serde_json::Value;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::error::{IssueCode, StringValidation, VldError};
use crate::schema::VldSchema;

use super::iso::{is_valid_iso_date, is_valid_iso_datetime, is_valid_iso_time};

// ---------------------------------------------------------------------------
// Manual validation functions (no regex)
// ---------------------------------------------------------------------------

fn is_valid_email(s: &str) -> bool {
    // local@domain — basic RFC-like check
    let Some((local, domain)) = s.rsplit_once('@') else {
        return false;
    };
    if local.is_empty() || local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return false;
    }
    // Local part: printable ASCII except some specials, no spaces
    local
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(ch))
        && is_valid_hostname(domain)
}

fn is_valid_uuid(s: &str) -> bool {
    // 8-4-4-4-12 hex with dashes
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

#[cfg(feature = "string-advanced")]
fn is_valid_uuid_version(s: &str, version: usize) -> bool {
    uuid::Uuid::parse_str(s).is_ok_and(|u| u.get_version_num() == version)
}

fn is_valid_url(s: &str) -> bool {
    let rest = match s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
    {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty() && !rest.contains(char::is_whitespace)
}

#[cfg(feature = "string-advanced")]
fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    !(v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast())
}

#[cfg(feature = "string-advanced")]
fn is_valid_url_strict(s: &str) -> bool {
    let parsed = match url::Url::parse(s) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    match parsed.host() {
        None => false,
        Some(url::Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d != "localhost" && !d.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(v4)) => is_public_ipv4(v4),
        Some(url::Host::Ipv6(v6)) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_ipv4(v4);
            }
            let seg0 = v6.segments()[0];
            let is_link_local = (seg0 & 0xffc0) == 0xfe80; // fe80::/10
            let is_unique_local = (seg0 & 0xfe00) == 0xfc00; // fc00::/7
            !(v6.is_loopback() || v6.is_unspecified() || is_link_local || is_unique_local)
        }
    }
}

#[cfg(feature = "string-advanced")]
fn is_valid_uri(s: &str) -> bool {
    url::Url::parse(s).is_ok()
}

fn is_valid_ipv4(s: &str) -> bool {
    s.parse::<Ipv4Addr>().is_ok()
}

fn is_valid_ipv6(s: &str) -> bool {
    s.parse::<Ipv6Addr>().is_ok()
}

fn is_valid_ip(s: &str) -> bool {
    is_valid_ipv4(s) || is_valid_ipv6(s)
}

fn is_valid_cidr(s: &str) -> bool {
    let Some((ip, prefix)) = s.split_once('/') else {
        return false;
    };
    // Digits only, no sign, no leading zeros.
    if prefix.is_empty()
        || prefix.len() > 3
        || !prefix.bytes().all(|b| b.is_ascii_digit())
        || (prefix.len() > 1 && prefix.starts_with('0'))
    {
        return false;
    }
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    if is_valid_ipv4(ip) {
        prefix <= 32
    } else {
        is_valid_ipv6(ip) && prefix <= 128
    }
}

fn is_valid_mac(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn is_valid_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_credit_card(s: &str) -> bool {
    // Only digits with optional space/dash separators.
    if !s
        .bytes()
        .all(|b| b.is_ascii_digit() || b == b' ' || b == b'-')
    {
        return false;
    }
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 12 || digits.len() > 19 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| match (i % 2 == 1, d * 2) {
            (true, v) if v > 9 => v - 9,
            (true, v) => v,
            (false, _) => d,
        })
        .sum();
    sum % 10 == 0
}

fn is_valid_phone(s: &str) -> bool {
    // Leading `+`, then digits with optional space / dash / parenthesis separators.
    let Some(rest) = s.strip_prefix('+') else {
        return false;
    };
    if !rest
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b' ' | b'-' | b'(' | b')'))
    {
        return false;
    }
    // E.164: max 15 digits, min practical 8.
    (8..=15).contains(&rest.bytes().filter(u8::is_ascii_digit).count())
}

#[cfg(feature = "string-advanced")]
fn is_valid_phone_e164_strict(s: &str) -> bool {
    phonenumber::parse(None, s).is_ok_and(|n| phonenumber::is_valid(&n))
}

fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_valid_color(s: &str) -> bool {
    if let Some(hex) = s.strip_prefix('#') {
        return (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    let lower = s.to_ascii_lowercase();
    let parts_of = |prefix: &str| -> Option<Vec<String>> {
        let inner = lower.strip_prefix(prefix)?.strip_suffix(')')?;
        let parts: Vec<String> = inner.split(',').map(|p| p.trim().to_string()).collect();
        (parts.len() == 3).then_some(parts)
    };
    if let Some(parts) = parts_of("rgb(") {
        return parts.iter().all(|p| p.parse::<u8>().is_ok());
    }
    if let Some(parts) = parts_of("hsl(") {
        let pct = |p: &str| {
            p.strip_suffix('%')
                .and_then(|v| v.parse::<u8>().ok())
                .is_some_and(|v| v <= 100)
        };
        return parts[0].parse::<u16>().is_ok_and(|h| h <= 360) && pct(&parts[1]) && pct(&parts[2]);
    }
    false
}

fn is_valid_currency_code(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase())
}

fn is_valid_country_code(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_uppercase())
}

fn is_valid_locale(s: &str) -> bool {
    let b = s.as_bytes();
    match b.len() {
        2 => b.iter().all(u8::is_ascii_lowercase),
        5 => {
            b[2] == b'-'
                && b[..2].iter().all(u8::is_ascii_lowercase)
                && b[3..].iter().all(u8::is_ascii_uppercase)
        }
        _ => false,
    }
}

fn is_valid_cron(s: &str) -> bool {
    let fields: Vec<&str> = s.split_whitespace().collect();
    (fields.len() == 5 || fields.len() == 6)
        && fields.iter().all(|f| {
            f.bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'*' | b'/' | b',' | b'-' | b'?'))
        })
}

fn is_valid_semver(s: &str) -> bool {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let main = s.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = main.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(feature = "string-advanced")]
fn is_valid_semver_full(s: &str) -> bool {
    semver::Version::parse(s).is_ok()
}

fn is_valid_base64_url(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn is_valid_jwt(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| is_valid_base64_url(p))
}

fn is_valid_base64(s: &str) -> bool {
    super::bytes::decode_base64(s).is_some()
}

fn is_valid_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

fn is_valid_cuid2(s: &str) -> bool {
    (2..=32).contains(&s.len())
        && s.as_bytes()[0].is_ascii_lowercase()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn is_valid_ulid(s: &str) -> bool {
    // Crockford's Base32: 0-9 A-H J K M N P-T V-Z (no I, L, O, U)
    s.len() == 26
        && s.chars().all(|c| {
            matches!(c.to_ascii_uppercase(), '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z')
        })
}

fn is_valid_nanoid(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_emoji_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x1F600..=0x1F64F
        | 0x1F300..=0x1F5FF
        | 0x1F680..=0x1F6FF
        | 0x1F1E0..=0x1F1FF
        | 0x2702..=0x27B0
        | 0x2600..=0x26FF
        | 0xFE00..=0xFE0F
        | 0x1F900..=0x1F9FF
        | 0x1FA00..=0x1FAFF
        | 0x231A..=0x231B
        | 0x23E9..=0x23F3
        | 0x23F8..=0x23FA
        | 0xE0020..=0xE007F // tag sequences (subdivision flags)
        | 0x20E3 // combining keycap
        | 0x200D
        | 0x2B50)
}

/// Non-empty and made of emoji characters only (ZWJ / variation selectors allowed).
fn is_valid_emoji(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_emoji_char)
}

/// Escape a literal for an ECMA-262 (`u` mode) JSON Schema `pattern`.
#[cfg(feature = "openapi")]
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "^$\\.*+?()[]{}|/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Declares every predicate-based string format exactly once: the enum
/// variant, its builder pair (`name()` / `name_msg()`), the `with_messages`
/// key, the [`StringValidation`] code, the JSON Schema `format` and the predicate.
macro_rules! string_formats {
    ($(
        $(#[cfg($cfg:meta)])?
        $variant:ident: $method:ident / $method_msg:ident,
        $key:literal, $schema_fmt:literal, $check:expr, $default:literal, $doc:literal;
    )+) => {
        #[derive(Clone, Copy)]
        enum Format {
            $( $(#[cfg($cfg)])? $variant, )+
        }

        impl Format {
            fn key(self) -> &'static str {
                match self { $( $(#[cfg($cfg)])? Format::$variant => $key, )+ }
            }

            #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
            fn schema_format(self) -> &'static str {
                match self { $( $(#[cfg($cfg)])? Format::$variant => $schema_fmt, )+ }
            }

            fn validation(self) -> StringValidation {
                match self { $( $(#[cfg($cfg)])? Format::$variant => StringValidation::$variant, )+ }
            }

            fn check(self, s: &str) -> bool {
                match self { $( $(#[cfg($cfg)])? Format::$variant => ($check)(s), )+ }
            }
        }

        impl ZString {
            $(
                #[doc = $doc]
                $(#[cfg($cfg)])?
                pub fn $method(self) -> Self {
                    self.$method_msg($default)
                }

                #[doc = $doc]
                #[doc = ""]
                #[doc = "With a custom error message."]
                $(#[cfg($cfg)])?
                pub fn $method_msg(self, msg: impl Into<String>) -> Self {
                    self.push(Kind::Format(Format::$variant), msg)
                }
            )+
        }
    };
}

string_formats! {
    Email: email / email_msg, "invalid_email", "email", is_valid_email,
        "Invalid email address", "Must be a valid email address.";
    Url: url / url_msg, "invalid_url", "uri", is_valid_url,
        "Invalid URL", "Must be a valid URL (http/https).";
    #[cfg(feature = "string-advanced")]
    UrlStrict: url_strict / url_strict_msg, "invalid_url_strict", "url-strict", is_valid_url_strict,
        "Invalid strict URL",
        "Must be a strict public URL: http/https, real host, not localhost / private / loopback / link-local / unspecified (IPv4-mapped IPv6 included).";
    #[cfg(feature = "string-advanced")]
    Uri: uri / uri_msg, "invalid_uri", "uri", is_valid_uri, "Invalid URI", "Must be a valid URI.";
    Uuid: uuid / uuid_msg, "invalid_uuid", "uuid", is_valid_uuid, "Invalid UUID", "Must be a valid UUID.";
    #[cfg(feature = "string-advanced")]
    UuidV1: uuid_v1 / uuid_v1_msg, "invalid_uuid_v1", "uuid-v1", |s| is_valid_uuid_version(s, 1),
        "Invalid UUID v1", "Must be a valid UUID v1.";
    #[cfg(feature = "string-advanced")]
    UuidV4: uuid_v4 / uuid_v4_msg, "invalid_uuid_v4", "uuid-v4", |s| is_valid_uuid_version(s, 4),
        "Invalid UUID v4", "Must be a valid UUID v4.";
    #[cfg(feature = "string-advanced")]
    UuidV7: uuid_v7 / uuid_v7_msg, "invalid_uuid_v7", "uuid-v7", |s| is_valid_uuid_version(s, 7),
        "Invalid UUID v7", "Must be a valid UUID v7.";
    Ip: ip / ip_msg, "invalid_ip", "ip", is_valid_ip,
        "Invalid IP address", "Must be a valid IP address (IPv4 or IPv6).";
    Slug: slug / slug_msg, "invalid_slug", "slug", is_valid_slug,
        "Invalid slug", "Must be a URL-friendly slug (`[a-z0-9-]`).";
    Color: color / color_msg, "invalid_color", "color", is_valid_color,
        "Invalid color", "Must be a color (`#RRGGBB`, `#RRGGBBAA`, `rgb(...)`, `hsl(...)`).";
    Currency: currency_code / currency_code_msg, "invalid_currency", "currency-code", is_valid_currency_code,
        "Invalid currency code", "Must be a currency code (ISO-4217-like uppercase 3 letters).";
    CountryCode: country_code / country_code_msg, "invalid_country_code", "country-code", is_valid_country_code,
        "Invalid country code", "Must be a country code (ISO-3166 alpha-2).";
    Locale: locale / locale_msg, "invalid_locale", "locale", is_valid_locale,
        "Invalid locale", "Must be a locale (`ll` or `ll-RR`, example: `en`, `en-US`).";
    Cron: cron / cron_msg, "invalid_cron", "cron", is_valid_cron,
        "Invalid cron expression", "Must be a cron expression (5 or 6 fields).";
    Ipv4: ipv4 / ipv4_msg, "invalid_ipv4", "ipv4", is_valid_ipv4,
        "Invalid IPv4 address", "Must be a valid IPv4 address.";
    Ipv6: ipv6 / ipv6_msg, "invalid_ipv6", "ipv6", is_valid_ipv6,
        "Invalid IPv6 address", "Must be a valid IPv6 address (IPv4-mapped forms included).";
    Cidr: cidr / cidr_msg, "invalid_cidr", "cidr", is_valid_cidr,
        "Invalid CIDR", "Must be a valid CIDR block (IPv4/IPv6 with a decimal prefix).";
    Mac: mac / mac_msg, "invalid_mac", "mac", is_valid_mac,
        "Invalid MAC address", "Must be a valid MAC address (`xx:xx:xx:xx:xx:xx`).";
    Hex: hex / hex_msg, "invalid_hex", "hex", is_valid_hex,
        "Invalid hexadecimal string", "Must contain only hexadecimal characters.";
    CreditCard: credit_card / credit_card_msg, "invalid_credit_card", "credit-card", is_valid_credit_card,
        "Invalid credit card number",
        "Must be a valid credit card number: digits with optional spaces/dashes, 12-19 digits, Luhn check.";
    Phone: phone / phone_msg, "invalid_phone", "phone", is_valid_phone,
        "Invalid phone number",
        "Must be an international phone number: leading `+`, 8-15 digits, optional spaces, dashes and parentheses.";
    #[cfg(feature = "string-advanced")]
    PhoneE164: phone_e164_strict / phone_e164_strict_msg, "invalid_phone_e164", "phone-e164", is_valid_phone_e164_strict,
        "Invalid E.164 phone number", "Must be a valid strict E.164 phone number.";
    Semver: semver / semver_msg, "invalid_semver", "semver", is_valid_semver,
        "Invalid semver", "Must be a semantic version (`MAJOR.MINOR.PATCH`, optional `v` prefix).";
    #[cfg(feature = "string-advanced")]
    SemverFull: semver_full / semver_full_msg, "invalid_semver_full", "semver-full", is_valid_semver_full,
        "Invalid semantic version", "Must be a full semantic version parsed by the `semver` crate.";
    Jwt: jwt / jwt_msg, "invalid_jwt", "jwt", is_valid_jwt,
        "Invalid JWT", "Must be a JWT-like token (`header.payload.signature`, base64url parts).";
    Ascii: ascii / ascii_msg, "invalid_ascii", "ascii", |s: &str| s.is_ascii(),
        "String must contain only ASCII characters", "Must contain only ASCII characters.";
    Alpha: alpha / alpha_msg, "invalid_alpha", "alpha", |s: &str| s.chars().all(|c| c.is_ascii_alphabetic()),
        "String must contain only alphabetic characters", "Must contain only alphabetic ASCII characters.";
    Alphanumeric: alphanumeric / alphanumeric_msg, "invalid_alphanumeric", "alphanumeric",
        |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric()),
        "String must contain only alphanumeric characters", "Must contain only ASCII alphanumeric characters.";
    Lowercase: lowercase / lowercase_msg, "invalid_lowercase", "lowercase", |s: &str| s == s.to_lowercase(),
        "String must be lowercase", "Must already be lowercase.";
    Uppercase: uppercase / uppercase_msg, "invalid_uppercase", "uppercase", |s: &str| s == s.to_uppercase(),
        "String must be uppercase", "Must already be uppercase.";
    Base64: base64 / base64_msg, "invalid_base64", "byte", is_valid_base64,
        "Invalid Base64 string", "Must be a valid (padded) Base64 string.";
    IsoDate: iso_date / iso_date_msg, "invalid_iso_date", "date", is_valid_iso_date,
        "Invalid ISO date (expected YYYY-MM-DD)", "Must be a valid ISO 8601 calendar date (`YYYY-MM-DD`, real day of month).";
    IsoDatetime: iso_datetime / iso_datetime_msg, "invalid_iso_datetime", "date-time", is_valid_iso_datetime,
        "Invalid ISO datetime",
        "Must be a valid ISO 8601 datetime: `YYYY-MM-DDTHH:MM[:SS[.frac]]` with optional `Z` / `±HH:MM`.";
    IsoTime: iso_time / iso_time_msg, "invalid_iso_time", "time", is_valid_iso_time,
        "Invalid ISO time", "Must be a valid ISO 8601 time (`HH:MM[:SS[.frac]]`).";
    Hostname: hostname / hostname_msg, "invalid_hostname", "hostname", is_valid_hostname,
        "Invalid hostname", "Must be a valid hostname.";
    Cuid2: cuid2 / cuid2_msg, "invalid_cuid2", "cuid2", is_valid_cuid2,
        "Invalid CUID2", "Must be a valid CUID2 (2-32 lowercase alphanumerics, starting with a letter).";
    Ulid: ulid / ulid_msg, "invalid_ulid", "ulid", is_valid_ulid,
        "Invalid ULID", "Must be a valid ULID (26-char Crockford Base32).";
    Nanoid: nanoid / nanoid_msg, "invalid_nanoid", "nanoid", is_valid_nanoid,
        "Invalid Nano ID", "Must be a valid Nano ID (alphanumeric + `_` + `-`).";
    Emoji: emoji / emoji_msg, "invalid_emoji", "emoji", is_valid_emoji,
        "String must contain only emoji", "Must be non-empty and consist only of emoji.";
}

#[derive(Clone)]
enum Kind {
    Min(usize),
    Max(usize),
    Len(usize),
    NonEmpty,
    StartsWith(String),
    EndsWith(String),
    Contains(String),
    #[cfg(feature = "regex")]
    Regex(regex_lite::Regex),
    Format(Format),
}

#[derive(Clone)]
struct StringCheck {
    kind: Kind,
    msg: String,
}

impl Kind {
    /// Stable key identifying the check category (for `with_messages`).
    fn key(&self) -> &'static str {
        match self {
            Kind::Min(_) => "too_small",
            Kind::Max(_) => "too_big",
            Kind::Len(_) => "invalid_length",
            Kind::NonEmpty => "non_empty",
            Kind::StartsWith(_) => "invalid_starts_with",
            Kind::EndsWith(_) => "invalid_ends_with",
            Kind::Contains(_) => "invalid_contains",
            #[cfg(feature = "regex")]
            Kind::Regex(_) => "invalid_regex",
            Kind::Format(f) => f.key(),
        }
    }

    /// `None` when the check passes, otherwise the issue code to report.
    fn failure(&self, s: &str) -> Option<IssueCode> {
        let invalid = |validation| Some(IssueCode::InvalidString { validation });
        match self {
            Kind::Min(n) => (s.chars().count() < *n).then_some(IssueCode::TooSmall {
                minimum: *n as f64,
                inclusive: true,
            }),
            Kind::Max(n) => (s.chars().count() > *n).then_some(IssueCode::TooBig {
                maximum: *n as f64,
                inclusive: true,
            }),
            Kind::Len(n) => (s.chars().count() != *n).then(|| IssueCode::Custom {
                code: "invalid_length".to_string(),
            }),
            Kind::NonEmpty => s.is_empty().then_some(IssueCode::TooSmall {
                minimum: 1.0,
                inclusive: true,
            }),
            Kind::StartsWith(p) if !s.starts_with(p.as_str()) => {
                invalid(StringValidation::StartsWith)
            }
            Kind::EndsWith(p) if !s.ends_with(p.as_str()) => invalid(StringValidation::EndsWith),
            // ponytail: `StringValidation` (error.rs) has no `Contains` variant yet,
            // so this stays a custom `invalid_string` code.
            Kind::Contains(p) if !s.contains(p.as_str()) => Some(IssueCode::Custom {
                code: "invalid_string".to_string(),
            }),
            #[cfg(feature = "regex")]
            Kind::Regex(re) if !re.is_match(s) => invalid(StringValidation::Regex),
            Kind::Format(f) if !f.check(s) => invalid(f.validation()),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum StringTransform {
    Trim,
    ToLowerCase,
    ToUpperCase,
}

/// Schema for string validation. Created via [`vld::string()`](crate::string).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(3).max(20).email();
/// ```
#[derive(Clone)]
pub struct ZString {
    checks: Vec<StringCheck>,
    transforms: Vec<StringTransform>,
    coerce: bool,
    custom_type_error: Option<String>,
}

impl ZString {
    pub fn new() -> Self {
        Self {
            checks: vec![],
            transforms: vec![],
            coerce: false,
            custom_type_error: None,
        }
    }

    fn push(mut self, kind: Kind, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck {
            kind,
            msg: msg.into(),
        });
        self
    }

    /// Set a custom error message for type mismatch (when the input is not a string).
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::string().type_error("Must be text!");
    /// let err = schema.parse("42").unwrap_err();
    /// assert!(err.issues[0].message.contains("Must be text!"));
    /// ```
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Override error messages in bulk by check key.
    ///
    /// The closure receives the check key (e.g. `"too_small"`, `"invalid_email"`)
    /// and should return `Some(new_message)` to replace, or `None` to keep the original.
    ///
    /// Keys: `"too_small"`, `"too_big"`, `"invalid_length"`, `"non_empty"`,
    /// `"invalid_regex"`, `"invalid_starts_with"`, `"invalid_ends_with"`,
    /// `"invalid_contains"`, and `"invalid_<format>"` for every format check
    /// (`"invalid_email"`, `"invalid_url"`, `"invalid_uuid"`, `"invalid_ip"`,
    /// `"invalid_currency"`, `"invalid_phone_e164"`, …).
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::string().min(3).email()
    ///     .with_messages(|key| match key {
    ///         "too_small" => Some("Too short!".into()),
    ///         "invalid_email" => Some("Bad email!".into()),
    ///         _ => None,
    ///     });
    /// ```
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        for check in &mut self.checks {
            if let Some(msg) = f(check.kind.key()) {
                check.msg = msg;
            }
        }
        self
    }

    /// Minimum string length in characters (inclusive).
    pub fn min(self, len: usize) -> Self {
        self.min_msg(len, format!("String must be at least {} characters", len))
    }

    /// Minimum string length with custom message.
    pub fn min_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Min(len), msg)
    }

    /// Maximum string length in characters (inclusive).
    pub fn max(self, len: usize) -> Self {
        self.max_msg(len, format!("String must be at most {} characters", len))
    }

    /// Maximum string length with custom message.
    pub fn max_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Max(len), msg)
    }

    /// Exact string length in characters.
    pub fn len(self, len: usize) -> Self {
        self.len_msg(len, format!("String must be exactly {} characters", len))
    }

    /// Exact string length with custom message.
    pub fn len_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Len(len), msg)
    }

    /// Must match the given regex (emitted as JSON Schema `pattern`).
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    pub fn regex(self, re: regex_lite::Regex) -> Self {
        self.regex_msg(re, "String does not match pattern")
    }

    /// Must match the given regex, with custom message.
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    pub fn regex_msg(self, re: regex_lite::Regex, msg: impl Into<String>) -> Self {
        self.push(Kind::Regex(re), msg)
    }

    /// Must start with the given prefix.
    pub fn starts_with(self, prefix: impl Into<String>) -> Self {
        let p = prefix.into();
        let msg = format!("String must start with \"{}\"", p);
        self.starts_with_msg(p, msg)
    }

    /// Must start with the given prefix, with custom message.
    pub fn starts_with_msg(self, prefix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::StartsWith(prefix.into()), msg)
    }

    /// Must end with the given suffix.
    pub fn ends_with(self, suffix: impl Into<String>) -> Self {
        let s = suffix.into();
        let msg = format!("String must end with \"{}\"", s);
        self.ends_with_msg(s, msg)
    }

    /// Must end with the given suffix, with custom message.
    pub fn ends_with_msg(self, suffix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::EndsWith(suffix.into()), msg)
    }

    /// Must contain the given substring.
    pub fn contains(self, sub: impl Into<String>) -> Self {
        let s = sub.into();
        let msg = format!("String must contain \"{}\"", s);
        self.contains_msg(s, msg)
    }

    /// Must contain the given substring, with custom message.
    pub fn contains_msg(self, sub: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::Contains(sub.into()), msg)
    }

    /// Must not be empty.
    pub fn non_empty(self) -> Self {
        self.non_empty_msg("String must not be empty")
    }

    /// Must not be empty, with custom message.
    pub fn non_empty_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::NonEmpty, msg)
    }

    /// Trim whitespace before validation.
    pub fn trim(mut self) -> Self {
        self.transforms.push(StringTransform::Trim);
        self
    }

    /// Convert to lowercase before validation.
    pub fn to_lowercase(mut self) -> Self {
        self.transforms.push(StringTransform::ToLowerCase);
        self
    }

    /// Convert to uppercase before validation.
    pub fn to_uppercase(mut self) -> Self {
        self.transforms.push(StringTransform::ToUpperCase);
        self
    }

    /// Coerce non-string values (numbers, booleans) to string.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }

    /// Generate a JSON Schema representation of this string schema.
    ///
    /// - Length bounds: the strictest `min`/`max`/`len`/`non_empty` wins.
    /// - Formats: the first goes into `format`, further ones into `allOf`.
    /// - `regex`/`starts_with`/`ends_with`/`contains` become `pattern`s
    ///   (first in `pattern`, further ones in `allOf`).
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string"});
        let (mut mins, mut maxs) = (vec![], vec![]);
        let mut formats: Vec<&str> = vec![];
        let mut patterns: Vec<String> = vec![];
        for check in &self.checks {
            match &check.kind {
                Kind::Min(n) => mins.push(*n),
                Kind::Max(n) => maxs.push(*n),
                Kind::Len(n) => {
                    mins.push(*n);
                    maxs.push(*n);
                }
                Kind::NonEmpty => mins.push(1),
                Kind::StartsWith(p) => patterns.push(format!("^{}", regex_escape(p))),
                Kind::EndsWith(p) => patterns.push(format!("{}$", regex_escape(p))),
                Kind::Contains(p) => patterns.push(regex_escape(p)),
                #[cfg(feature = "regex")]
                Kind::Regex(re) => patterns.push(re.as_str().to_string()),
                Kind::Format(f) => {
                    if !formats.contains(&f.schema_format()) {
                        formats.push(f.schema_format());
                    }
                }
            }
        }
        super::length_schema(&mut schema, ("minLength", "maxLength"), mins, maxs);
        let mut all_of = vec![];
        let mut formats = formats.into_iter();
        if let Some(f) = formats.next() {
            schema["format"] = serde_json::json!(f);
        }
        all_of.extend(formats.map(|f| serde_json::json!({ "format": f })));
        let mut patterns = patterns.into_iter();
        if let Some(p) = patterns.next() {
            schema["pattern"] = serde_json::json!(p);
        }
        all_of.extend(patterns.map(|p| serde_json::json!({ "pattern": p })));
        if !all_of.is_empty() {
            schema["allOf"] = serde_json::Value::Array(all_of);
        }
        schema
    }
}

impl Default for ZString {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZString {
    type Output = String;

    fn parse_value(&self, value: &Value) -> Result<String, VldError> {
        let type_err = || super::type_error(&self.custom_type_error, "string", "string", value);

        let mut s = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) if self.coerce => n.to_string(),
            Value::Bool(b) if self.coerce => b.to_string(),
            _ => return Err(type_err()),
        };

        for t in &self.transforms {
            s = match t {
                StringTransform::Trim => s.trim().to_string(),
                StringTransform::ToLowerCase => s.to_lowercase(),
                StringTransform::ToUpperCase => s.to_uppercase(),
            };
        }

        let str_val = Value::String(s.clone());
        let mut errors = VldError::new();
        for check in &self.checks {
            if let Some(code) = check.kind.failure(&s) {
                errors.push_with_value(code, check.msg.clone(), &str_val);
            }
        }

        if errors.is_empty() {
            Ok(s)
        } else {
            Err(errors)
        }
    }
}
