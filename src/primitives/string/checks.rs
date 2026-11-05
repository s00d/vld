//! The string check table: every format check is declared once in
//! `string_formats!` (builder methods, `with_messages` key, issue code,
//! JSON Schema `format`, predicate).

use crate::error::{IssueCode, SizeOrigin, StringValidation};
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
        $key:literal, $schema_fmt:literal, $check:expr, $doc:literal;
    )+) => {
        #[derive(Clone, Copy)]
        pub(super) enum Format {
            $( $(#[cfg($cfg)])? $variant, )+
        }

        impl Format {
            fn key(self) -> &'static str {
                match self { $( $(#[cfg($cfg)])? Format::$variant => $key, )+ }
            }

            #[cfg(feature = "openapi")]
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
                    self.push(Kind::Format(Format::$variant), None)
                }

                #[doc = $doc]
                #[doc = ""]
                #[doc = "With a custom error message."]
                $(#[cfg($cfg)])?
                pub fn $method_msg(self, msg: impl Into<String>) -> Self {
                    self.push(Kind::Format(Format::$variant), Some(msg.into()))
                }
            )+
        }
    };
}

string_formats! {
    Email: email / email_msg, "invalid_email", "email", is_valid_email, "Must be a valid email address.";
    Url: url / url_msg, "invalid_url", "uri", is_valid_url, "Must be a valid URL (http/https).";
    #[cfg(feature = "string-advanced")]
    UrlStrict: url_strict / url_strict_msg, "invalid_url_strict", "url-strict", is_valid_url_strict,
        "Must be a strict public URL: http/https, real host, not localhost / private / loopback / link-local / unspecified (IPv4-mapped IPv6 included).";
    #[cfg(feature = "string-advanced")]
    Uri: uri / uri_msg, "invalid_uri", "uri", is_valid_uri, "Must be a valid URI.";
    Uuid: uuid / uuid_msg, "invalid_uuid", "uuid", is_valid_uuid, "Must be a valid UUID.";
    #[cfg(feature = "string-advanced")]
    UuidV1: uuid_v1 / uuid_v1_msg, "invalid_uuid_v1", "uuid-v1", |s| is_valid_uuid_version(s, 1), "Must be a valid UUID v1.";
    #[cfg(feature = "string-advanced")]
    UuidV4: uuid_v4 / uuid_v4_msg, "invalid_uuid_v4", "uuid-v4", |s| is_valid_uuid_version(s, 4), "Must be a valid UUID v4.";
    #[cfg(feature = "string-advanced")]
    UuidV7: uuid_v7 / uuid_v7_msg, "invalid_uuid_v7", "uuid-v7", |s| is_valid_uuid_version(s, 7), "Must be a valid UUID v7.";
    Ip: ip / ip_msg, "invalid_ip", "ip", is_valid_ip, "Must be a valid IP address (IPv4 or IPv6).";
    Slug: slug / slug_msg, "invalid_slug", "slug", is_valid_slug, "Must be a URL-friendly slug (`[a-z0-9-]`).";
    Color: color / color_msg, "invalid_color", "color", is_valid_color, "Must be a color (`#RRGGBB`, `#RRGGBBAA`, `rgb(...)`, `hsl(...)`).";
    Currency: currency_code / currency_code_msg, "invalid_currency", "currency-code", is_valid_currency_code, "Must be a currency code (ISO-4217-like uppercase 3 letters).";
    CountryCode: country_code / country_code_msg, "invalid_country_code", "country-code", is_valid_country_code, "Must be a country code (ISO-3166 alpha-2).";
    Locale: locale / locale_msg, "invalid_locale", "locale", is_valid_locale, "Must be a locale (`ll` or `ll-RR`, example: `en`, `en-US`).";
    Cron: cron / cron_msg, "invalid_cron", "cron", is_valid_cron, "Must be a cron expression (5 or 6 fields).";
    Ipv4: ipv4 / ipv4_msg, "invalid_ipv4", "ipv4", is_valid_ipv4, "Must be a valid IPv4 address.";
    Ipv6: ipv6 / ipv6_msg, "invalid_ipv6", "ipv6", is_valid_ipv6, "Must be a valid IPv6 address (IPv4-mapped forms included).";
    Cidr: cidr / cidr_msg, "invalid_cidr", "cidr", is_valid_cidr, "Must be a valid CIDR block (IPv4/IPv6 with a decimal prefix).";
    Mac: mac / mac_msg, "invalid_mac", "mac", is_valid_mac, "Must be a valid MAC address (`xx:xx:xx:xx:xx:xx`).";
    Hex: hex / hex_msg, "invalid_hex", "hex", is_valid_hex, "Must contain only hexadecimal characters.";
    CreditCard: credit_card / credit_card_msg, "invalid_credit_card", "credit-card", is_valid_credit_card,
        "Must be a valid credit card number: digits with optional spaces/dashes, 12-19 digits, Luhn check.";
    Phone: phone / phone_msg, "invalid_phone", "phone", is_valid_phone,
        "Must be an international phone number: leading `+`, 8-15 digits, optional spaces, dashes and parentheses.";
    #[cfg(feature = "string-advanced")]
    PhoneE164: phone_e164_strict / phone_e164_strict_msg, "invalid_phone_e164", "phone-e164", is_valid_phone_e164_strict, "Must be a valid strict E.164 phone number.";
    Semver: semver / semver_msg, "invalid_semver", "semver", is_valid_semver, "Must be a semantic version (`MAJOR.MINOR.PATCH`, optional `v` prefix).";
    #[cfg(feature = "string-advanced")]
    SemverFull: semver_full / semver_full_msg, "invalid_semver_full", "semver-full", is_valid_semver_full, "Must be a full semantic version parsed by the `semver` crate.";
    Jwt: jwt / jwt_msg, "invalid_jwt", "jwt", is_valid_jwt, "Must be a JWT-like token (`header.payload.signature`, base64url parts).";
    Ascii: ascii / ascii_msg, "invalid_ascii", "ascii", |s: &str| s.is_ascii(), "Must contain only ASCII characters.";
    Alpha: alpha / alpha_msg, "invalid_alpha", "alpha", |s: &str| s.chars().all(|c| c.is_ascii_alphabetic()), "Must contain only alphabetic ASCII characters.";
    Alphanumeric: alphanumeric / alphanumeric_msg, "invalid_alphanumeric", "alphanumeric",
        |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric()), "Must contain only ASCII alphanumeric characters.";
    Lowercase: lowercase / lowercase_msg, "invalid_lowercase", "lowercase", |s: &str| s == s.to_lowercase(), "Must already be lowercase.";
    Uppercase: uppercase / uppercase_msg, "invalid_uppercase", "uppercase", |s: &str| s == s.to_uppercase(), "Must already be uppercase.";
    Base64: base64 / base64_msg, "invalid_base64", "byte", is_valid_base64, "Must be a valid (padded) Base64 string.";
    Base64Url: base64url / base64url_msg, "invalid_base64url", "base64url", is_valid_base64url, "Must be a valid Base64URL string (`-`/`_` alphabet, padding optional).";
    Json: json / json_msg, "invalid_json", "json", is_valid_json, "Must be a string containing one valid JSON document.";
    IsoDate: iso_date / iso_date_msg, "invalid_iso_date", "date", is_valid_iso_date, "Must be a valid ISO 8601 calendar date (`YYYY-MM-DD`, real day of month).";
    IsoDatetime: iso_datetime / iso_datetime_msg, "invalid_iso_datetime", "date-time", is_valid_iso_datetime,
        "Must be a valid ISO 8601 datetime: `YYYY-MM-DDTHH:MM:SS[.frac]` (seconds required, `T`/`t`) with optional `Z`/`z` / `±HH:MM` — the same grammar as `vld::chrono::datetime()` & co.";
    IsoTime: iso_time / iso_time_msg, "invalid_iso_time", "time", is_valid_iso_time, "Must be a valid ISO 8601 time (`HH:MM[:SS[.frac]]`).";
    Hostname: hostname / hostname_msg, "invalid_hostname", "hostname", is_valid_hostname, "Must be a valid hostname.";
    Cuid2: cuid2 / cuid2_msg, "invalid_cuid2", "cuid2", is_valid_cuid2, "Must be a valid CUID2 (2-32 lowercase alphanumerics, starting with a letter).";
    Ulid: ulid / ulid_msg, "invalid_ulid", "ulid", is_valid_ulid, "Must be a valid ULID (26-char Crockford Base32).";
    Nanoid: nanoid / nanoid_msg, "invalid_nanoid", "nanoid", is_valid_nanoid, "Must be a valid Nano ID (alphanumeric + `_` + `-`).";
    Emoji: emoji / emoji_msg, "invalid_emoji", "emoji", is_valid_emoji, "Must be non-empty and consist only of emoji.";
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
    /// Custom message; `None` renders the default for the issue code.
    pub(super) msg: Option<String>,
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
        let len = || s.chars().count();
        match self {
            Kind::Min(n) if len() < *n => Some(too_small(*n)),
            Kind::Max(n) if len() > *n => Some(IssueCode::TooBig {
                origin: SizeOrigin::String,
                maximum: (*n).into(),
                inclusive: true,
            }),
            Kind::Len(n) if len() != *n => Some(IssueCode::InvalidLength {
                origin: SizeOrigin::String,
                expected: *n,
            }),
            Kind::NonEmpty if s.is_empty() => Some(too_small(1)),
            Kind::StartsWith(p) if !s.starts_with(p.as_str()) => {
                invalid(StringValidation::StartsWith { prefix: p.clone() })
            }
            Kind::EndsWith(p) if !s.ends_with(p.as_str()) => {
                invalid(StringValidation::EndsWith { suffix: p.clone() })
            }
            Kind::Contains(p) if !s.contains(p.as_str()) => invalid(StringValidation::Contains {
                substring: p.clone(),
            }),
            #[cfg(feature = "regex")]
            Kind::Regex(re) if !re.is_match(s) => invalid(StringValidation::Regex),
            Kind::Format(f) if !f.check(s) => invalid(f.validation()),
            _ => None,
        }
    }
}

fn too_small(n: usize) -> IssueCode {
    IssueCode::TooSmall {
        origin: SizeOrigin::String,
        minimum: n.into(),
        inclusive: true,
    }
}
