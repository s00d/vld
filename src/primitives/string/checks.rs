//! The string check table: every format check is declared once in
//! `string_formats!` (builder methods, `with_messages` key, issue code,
//! JSON Schema `format`, predicate).

use crate::error::{IssueCode, StringValidation};
use crate::primitives::iso::{is_valid_iso_date, is_valid_iso_datetime, is_valid_iso_time};

use super::formats::*;
use super::ZString;

/// Escape a literal for an ECMA-262 (`u` mode) JSON Schema `pattern`.
#[cfg(feature = "openapi")]
pub(super) fn regex_escape(s: &str) -> String {
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
        pub(super) enum Format {
            $( $(#[cfg($cfg)])? $variant, )+
        }

        impl Format {
            fn key(self) -> &'static str {
                match self { $( $(#[cfg($cfg)])? Format::$variant => $key, )+ }
            }

            #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
            pub(super) fn schema_format(self) -> &'static str {
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
pub(super) enum Kind {
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
pub(super) struct StringCheck {
    pub(super) kind: Kind,
    pub(super) msg: String,
}

impl Kind {
    /// Stable key identifying the check category (for `with_messages`).
    pub(super) fn key(&self) -> &'static str {
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
    pub(super) fn failure(&self, s: &str) -> Option<IssueCode> {
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
