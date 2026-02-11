use serde_json::Value;

use crate::error::{value_type_name, IssueCode, StringValidation, VldError};
use crate::schema::VldSchema;

// ---------------------------------------------------------------------------
// Manual validation functions (no regex)
// ---------------------------------------------------------------------------

fn is_valid_email(s: &str) -> bool {
    // local@domain — basic RFC-like check
    let at = match s.find('@') {
        Some(pos) if pos > 0 => pos,
        _ => return false,
    };
    let local = &s[..at];
    let domain = &s[at + 1..];

    if local.is_empty() || domain.is_empty() {
        return false;
    }

    // Local part: printable ASCII except some specials, no spaces
    for ch in local.chars() {
        if ch.is_ascii_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(ch) {
            continue;
        }
        return false;
    }

    is_valid_hostname(domain)
}

fn is_valid_uuid(s: &str) -> bool {
    // 8-4-4-4-12 hex with dashes: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

fn is_valid_url(s: &str) -> bool {
    // Must start with http:// or https:// and have something after
    let rest = if let Some(r) = s.strip_prefix("https://") {
        r
    } else if let Some(r) = s.strip_prefix("http://") {
        r
    } else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    // No whitespace allowed
    !rest.contains(char::is_whitespace)
}

fn is_valid_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        // No leading zeros (except "0" itself)
        if part.len() > 1 && part.starts_with('0') {
            return false;
        }
        match part.parse::<u16>() {
            Ok(n) if n <= 255 => {}
            _ => return false,
        }
    }
    true
}

fn is_valid_ipv6(s: &str) -> bool {
    // Handle :: shorthand
    if s == "::" {
        return true;
    }

    let (left, right) = if let Some(pos) = s.find("::") {
        (&s[..pos], &s[pos + 2..])
    } else {
        (s, "")
    };

    let has_double_colon = s.contains("::");

    let left_groups: Vec<&str> = if left.is_empty() {
        vec![]
    } else {
        left.split(':').collect()
    };

    let right_groups: Vec<&str> = if right.is_empty() {
        vec![]
    } else {
        right.split(':').collect()
    };

    let total = left_groups.len() + right_groups.len();

    if has_double_colon {
        if total > 7 {
            return false;
        }
    } else if total != 8 {
        return false;
    }

    for group in left_groups.iter().chain(right_groups.iter()) {
        if group.is_empty() || group.len() > 4 {
            return false;
        }
        if !group.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }

    true
}

fn is_valid_base64(s: &str) -> bool {
    if s.is_empty() || s.len() % 4 != 0 {
        return false;
    }
    let mut pad_started = false;
    for &b in s.as_bytes() {
        if pad_started {
            if b != b'=' {
                return false;
            }
        } else if b == b'=' {
            pad_started = true;
        } else if !(b.is_ascii_alphanumeric() || b == b'+' || b == b'/') {
            return false;
        }
    }
    // At most 2 padding chars
    let pad_count = s.bytes().rev().take_while(|&b| b == b'=').count();
    pad_count <= 2
}

/// Parse exactly `n` ASCII digits from `s`, return the number and remaining slice.
fn parse_digits(s: &str, n: usize) -> Option<(u32, &str)> {
    if s.len() < n {
        return None;
    }
    let (digits, rest) = s.split_at(n);
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok().map(|v| (v, rest))
}

fn is_valid_iso_date(s: &str) -> bool {
    // YYYY-MM-DD (exactly 10 chars)
    if s.len() != 10 {
        return false;
    }
    let (year, rest) = match parse_digits(s, 4) {
        Some(v) => v,
        None => return false,
    };
    let rest = rest.strip_prefix('-').unwrap_or("");
    let (month, rest) = match parse_digits(rest, 2) {
        Some(v) => v,
        None => return false,
    };
    let rest = rest.strip_prefix('-').unwrap_or("");
    let (day, rest) = match parse_digits(rest, 2) {
        Some(v) => v,
        None => return false,
    };
    if !rest.is_empty() {
        return false;
    }
    let _ = year; // any 4-digit year is OK
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn is_valid_iso_time(s: &str) -> bool {
    // HH:MM[:SS[.frac]]
    if s.len() < 5 {
        return false;
    }
    let (hour, rest) = match parse_digits(s, 2) {
        Some(v) => v,
        None => return false,
    };
    let rest = match rest.strip_prefix(':') {
        Some(r) => r,
        None => return false,
    };
    let (min, rest) = match parse_digits(rest, 2) {
        Some(v) => v,
        None => return false,
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&min) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    // Optional :SS
    let rest = match rest.strip_prefix(':') {
        Some(r) => r,
        None => return false,
    };
    let (sec, rest) = match parse_digits(rest, 2) {
        Some(v) => v,
        None => return false,
    };
    if !(0..=59).contains(&sec) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    // Optional .fraction
    let rest = match rest.strip_prefix('.') {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

fn is_valid_iso_datetime(s: &str) -> bool {
    // YYYY-MM-DDTHH:MM[:SS[.frac]][Z|+HH:MM|-HH:MM]
    let t_pos = match s.find('T') {
        Some(p) => p,
        None => return false,
    };
    let date_part = &s[..t_pos];
    let after_t = &s[t_pos + 1..];

    if !is_valid_iso_date(date_part) {
        return false;
    }

    // Find timezone suffix
    let (time_part, _tz_part) = if let Some(pos) = after_t.rfind('Z') {
        (&after_t[..pos], &after_t[pos..])
    } else if let Some(pos) = after_t.rfind('+') {
        (&after_t[..pos], &after_t[pos..])
    } else if let Some(pos) = after_t[1..].rfind('-') {
        // skip first char to avoid matching negative in time itself
        let actual = pos + 1;
        (&after_t[..actual], &after_t[actual..])
    } else {
        (after_t, "")
    };

    is_valid_iso_time(time_part)
}

fn is_valid_hostname(s: &str) -> bool {
    if s.is_empty() || s.len() > 253 {
        return false;
    }
    for label in s.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    true
}

fn is_valid_cuid2(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if !s.as_bytes()[0].is_ascii_lowercase() {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn is_valid_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    // Crockford's Base32: 0-9 A-H J K M N P-T V-Z (no I, L, O, U)
    s.chars().all(|c| {
        let c = c.to_ascii_uppercase();
        matches!(c, '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z')
    })
}

fn is_valid_nanoid(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_valid_emoji(s: &str) -> bool {
    s.chars().any(|c| {
        let cp = c as u32;
        (0x1F600..=0x1F64F).contains(&cp)
            || (0x1F300..=0x1F5FF).contains(&cp)
            || (0x1F680..=0x1F6FF).contains(&cp)
            || (0x1F1E0..=0x1F1FF).contains(&cp)
            || (0x2702..=0x27B0).contains(&cp)
            || (0x2600..=0x26FF).contains(&cp)
            || (0xFE00..=0xFE0F).contains(&cp)
            || (0x1F900..=0x1F9FF).contains(&cp)
            || (0x1FA00..=0x1FA6F).contains(&cp)
            || (0x1FA70..=0x1FAFF).contains(&cp)
            || (0x231A..=0x231B).contains(&cp)
            || (0x23E9..=0x23F3).contains(&cp)
            || (0x23F8..=0x23FA).contains(&cp)
            || cp == 0x200D
            || cp == 0x2B50
            || cp == 0x2764
    })
}

// ---------------------------------------------------------------------------
// StringCheck / StringTransform
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum StringCheck {
    Min(usize, String),
    Max(usize, String),
    Len(usize, String),
    Email(String),
    Url(String),
    Uuid(String),
    #[cfg(feature = "regex")]
    Regex(regex_lite::Regex, String),
    StartsWith(String, String),
    EndsWith(String, String),
    Contains(String, String),
    NonEmpty(String),
    Ipv4(String),
    Ipv6(String),
    Base64(String),
    IsoDate(String),
    IsoDatetime(String),
    IsoTime(String),
    Hostname(String),
    Cuid2(String),
    Ulid(String),
    Nanoid(String),
    Emoji(String),
}

impl StringCheck {
    /// Stable key identifying the check category.
    fn key(&self) -> &str {
        match self {
            StringCheck::Min(..) => "too_small",
            StringCheck::Max(..) => "too_big",
            StringCheck::Len(..) => "invalid_length",
            StringCheck::Email(..) => "invalid_email",
            StringCheck::Url(..) => "invalid_url",
            StringCheck::Uuid(..) => "invalid_uuid",
            #[cfg(feature = "regex")]
            StringCheck::Regex(..) => "invalid_regex",
            StringCheck::StartsWith(..) => "invalid_starts_with",
            StringCheck::EndsWith(..) => "invalid_ends_with",
            StringCheck::Contains(..) => "invalid_contains",
            StringCheck::NonEmpty(..) => "non_empty",
            StringCheck::Ipv4(..) => "invalid_ipv4",
            StringCheck::Ipv6(..) => "invalid_ipv6",
            StringCheck::Base64(..) => "invalid_base64",
            StringCheck::IsoDate(..) => "invalid_iso_date",
            StringCheck::IsoDatetime(..) => "invalid_iso_datetime",
            StringCheck::IsoTime(..) => "invalid_iso_time",
            StringCheck::Hostname(..) => "invalid_hostname",
            StringCheck::Cuid2(..) => "invalid_cuid2",
            StringCheck::Ulid(..) => "invalid_ulid",
            StringCheck::Nanoid(..) => "invalid_nanoid",
            StringCheck::Emoji(..) => "invalid_emoji",
        }
    }

    /// Replace the error message stored in this check.
    fn set_message(&mut self, msg: String) {
        match self {
            StringCheck::Min(_, ref mut m)
            | StringCheck::Max(_, ref mut m)
            | StringCheck::Len(_, ref mut m)
            | StringCheck::Email(ref mut m)
            | StringCheck::Url(ref mut m)
            | StringCheck::Uuid(ref mut m)
            | StringCheck::NonEmpty(ref mut m)
            | StringCheck::Ipv4(ref mut m)
            | StringCheck::Ipv6(ref mut m)
            | StringCheck::Base64(ref mut m)
            | StringCheck::IsoDate(ref mut m)
            | StringCheck::IsoDatetime(ref mut m)
            | StringCheck::IsoTime(ref mut m)
            | StringCheck::Hostname(ref mut m)
            | StringCheck::Cuid2(ref mut m)
            | StringCheck::Ulid(ref mut m)
            | StringCheck::Nanoid(ref mut m)
            | StringCheck::Emoji(ref mut m) => *m = msg,
            StringCheck::StartsWith(_, ref mut m)
            | StringCheck::EndsWith(_, ref mut m)
            | StringCheck::Contains(_, ref mut m) => *m = msg,
            #[cfg(feature = "regex")]
            StringCheck::Regex(_, ref mut m) => *m = msg,
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
    /// Available keys: `"too_small"`, `"too_big"`, `"invalid_length"`, `"invalid_email"`,
    /// `"invalid_url"`, `"invalid_uuid"`, `"invalid_regex"`, `"invalid_starts_with"`,
    /// `"invalid_ends_with"`, `"invalid_contains"`, `"non_empty"`, `"invalid_ipv4"`,
    /// `"invalid_ipv6"`, `"invalid_base64"`, `"invalid_iso_date"`, `"invalid_iso_datetime"`,
    /// `"invalid_iso_time"`, `"invalid_hostname"`, `"invalid_cuid2"`, `"invalid_ulid"`,
    /// `"invalid_nanoid"`, `"invalid_emoji"`.
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
            if let Some(msg) = f(check.key()) {
                check.set_message(msg);
            }
        }
        self
    }

    /// Minimum string length (inclusive).
    pub fn min(self, len: usize) -> Self {
        self.min_msg(len, format!("String must be at least {} characters", len))
    }

    /// Minimum string length with custom message.
    pub fn min_msg(mut self, len: usize, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Min(len, msg.into()));
        self
    }

    /// Maximum string length (inclusive).
    pub fn max(self, len: usize) -> Self {
        self.max_msg(len, format!("String must be at most {} characters", len))
    }

    /// Maximum string length with custom message.
    pub fn max_msg(mut self, len: usize, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Max(len, msg.into()));
        self
    }

    /// Exact string length.
    pub fn len(self, len: usize) -> Self {
        self.len_msg(len, format!("String must be exactly {} characters", len))
    }

    /// Exact string length with custom message.
    pub fn len_msg(mut self, len: usize, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Len(len, msg.into()));
        self
    }

    /// Must be a valid email address.
    pub fn email(self) -> Self {
        self.email_msg("Invalid email address")
    }

    /// Must be a valid email address, with custom message.
    pub fn email_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Email(msg.into()));
        self
    }

    /// Must be a valid URL (http/https).
    pub fn url(self) -> Self {
        self.url_msg("Invalid URL")
    }

    /// Must be a valid URL, with custom message.
    pub fn url_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Url(msg.into()));
        self
    }

    /// Must be a valid UUID.
    pub fn uuid(self) -> Self {
        self.uuid_msg("Invalid UUID")
    }

    /// Must be a valid UUID, with custom message.
    pub fn uuid_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Uuid(msg.into()));
        self
    }

    /// Must match the given regex.
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
    pub fn regex_msg(mut self, re: regex_lite::Regex, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Regex(re, msg.into()));
        self
    }

    /// Must start with the given prefix.
    pub fn starts_with(self, prefix: impl Into<String>) -> Self {
        let p = prefix.into();
        let msg = format!("String must start with \"{}\"", p);
        self.starts_with_msg(p, msg)
    }

    /// Must start with the given prefix, with custom message.
    pub fn starts_with_msg(mut self, prefix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.checks
            .push(StringCheck::StartsWith(prefix.into(), msg.into()));
        self
    }

    /// Must end with the given suffix.
    pub fn ends_with(self, suffix: impl Into<String>) -> Self {
        let s = suffix.into();
        let msg = format!("String must end with \"{}\"", s);
        self.ends_with_msg(s, msg)
    }

    /// Must end with the given suffix, with custom message.
    pub fn ends_with_msg(mut self, suffix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.checks
            .push(StringCheck::EndsWith(suffix.into(), msg.into()));
        self
    }

    /// Must contain the given substring.
    pub fn contains(self, sub: impl Into<String>) -> Self {
        let s = sub.into();
        let msg = format!("String must contain \"{}\"", s);
        self.contains_msg(s, msg)
    }

    /// Must contain the given substring, with custom message.
    pub fn contains_msg(mut self, sub: impl Into<String>, msg: impl Into<String>) -> Self {
        self.checks
            .push(StringCheck::Contains(sub.into(), msg.into()));
        self
    }

    /// Must not be empty.
    pub fn non_empty(self) -> Self {
        self.non_empty_msg("String must not be empty")
    }

    /// Must not be empty, with custom message.
    pub fn non_empty_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::NonEmpty(msg.into()));
        self
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

    /// Must be a valid IPv4 address.
    pub fn ipv4(self) -> Self {
        self.ipv4_msg("Invalid IPv4 address")
    }

    /// Must be a valid IPv4 address, with custom message.
    pub fn ipv4_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Ipv4(msg.into()));
        self
    }

    /// Must be a valid IPv6 address.
    pub fn ipv6(self) -> Self {
        self.ipv6_msg("Invalid IPv6 address")
    }

    /// Must be a valid IPv6 address, with custom message.
    pub fn ipv6_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Ipv6(msg.into()));
        self
    }

    /// Must be a valid Base64 string.
    pub fn base64(self) -> Self {
        self.base64_msg("Invalid Base64 string")
    }

    /// Must be a valid Base64 string, with custom message.
    pub fn base64_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Base64(msg.into()));
        self
    }

    /// Must be a valid ISO 8601 date (YYYY-MM-DD).
    pub fn iso_date(self) -> Self {
        self.iso_date_msg("Invalid ISO date (expected YYYY-MM-DD)")
    }

    /// Must be a valid ISO 8601 date, with custom message.
    pub fn iso_date_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::IsoDate(msg.into()));
        self
    }

    /// Must be a valid ISO 8601 datetime.
    pub fn iso_datetime(self) -> Self {
        self.iso_datetime_msg("Invalid ISO datetime")
    }

    /// Must be a valid ISO 8601 datetime, with custom message.
    pub fn iso_datetime_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::IsoDatetime(msg.into()));
        self
    }

    /// Must be a valid ISO 8601 time (HH:MM or HH:MM:SS).
    pub fn iso_time(self) -> Self {
        self.iso_time_msg("Invalid ISO time")
    }

    /// Must be a valid ISO 8601 time, with custom message.
    pub fn iso_time_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::IsoTime(msg.into()));
        self
    }

    /// Must be a valid hostname.
    pub fn hostname(self) -> Self {
        self.hostname_msg("Invalid hostname")
    }

    /// Must be a valid hostname, with custom message.
    pub fn hostname_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Hostname(msg.into()));
        self
    }

    /// Must be a valid CUID2 string (lowercase alphanumeric, starts with a letter).
    pub fn cuid2(self) -> Self {
        self.cuid2_msg("Invalid CUID2")
    }

    /// Must be a valid CUID2 string, with custom message.
    pub fn cuid2_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Cuid2(msg.into()));
        self
    }

    /// Must be a valid ULID (26-char Crockford Base32).
    pub fn ulid(self) -> Self {
        self.ulid_msg("Invalid ULID")
    }

    /// Must be a valid ULID, with custom message.
    pub fn ulid_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Ulid(msg.into()));
        self
    }

    /// Must be a valid Nano ID (alphanumeric + `_` + `-`).
    pub fn nanoid(self) -> Self {
        self.nanoid_msg("Invalid Nano ID")
    }

    /// Must be a valid Nano ID, with custom message.
    pub fn nanoid_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Nanoid(msg.into()));
        self
    }

    /// Must contain at least one emoji character.
    pub fn emoji(self) -> Self {
        self.emoji_msg("String must contain an emoji")
    }

    /// Must contain an emoji, with custom message.
    pub fn emoji_msg(mut self, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck::Emoji(msg.into()));
        self
    }

    /// Generate a JSON Schema representation of this string schema.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string"});
        for check in &self.checks {
            match check {
                StringCheck::Min(n, _) => {
                    schema["minLength"] = serde_json::json!(*n);
                }
                StringCheck::Max(n, _) => {
                    schema["maxLength"] = serde_json::json!(*n);
                }
                StringCheck::Len(n, _) => {
                    schema["minLength"] = serde_json::json!(*n);
                    schema["maxLength"] = serde_json::json!(*n);
                }
                StringCheck::Email(_) => {
                    schema["format"] = serde_json::json!("email");
                }
                StringCheck::Url(_) => {
                    schema["format"] = serde_json::json!("uri");
                }
                StringCheck::Uuid(_) => {
                    schema["format"] = serde_json::json!("uuid");
                }
                StringCheck::Ipv4(_) => {
                    schema["format"] = serde_json::json!("ipv4");
                }
                StringCheck::Ipv6(_) => {
                    schema["format"] = serde_json::json!("ipv6");
                }
                StringCheck::IsoDate(_) => {
                    schema["format"] = serde_json::json!("date");
                }
                StringCheck::IsoDatetime(_) => {
                    schema["format"] = serde_json::json!("date-time");
                }
                StringCheck::IsoTime(_) => {
                    schema["format"] = serde_json::json!("time");
                }
                StringCheck::Hostname(_) => {
                    schema["format"] = serde_json::json!("hostname");
                }
                StringCheck::NonEmpty(_) => {
                    schema["minLength"] = serde_json::json!(1);
                }
                StringCheck::Cuid2(_) => {
                    schema["format"] = serde_json::json!("cuid2");
                }
                StringCheck::Ulid(_) => {
                    schema["format"] = serde_json::json!("ulid");
                }
                StringCheck::Nanoid(_) => {
                    schema["format"] = serde_json::json!("nanoid");
                }
                StringCheck::Emoji(_) => {
                    schema["format"] = serde_json::json!("emoji");
                }
                _ => {}
            }
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
        let type_err = |value: &Value| -> VldError {
            let msg = self
                .custom_type_error
                .clone()
                .unwrap_or_else(|| format!("Expected string, received {}", value_type_name(value)));
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        };

        // Extract string value
        let mut s = if let Some(s) = value.as_str() {
            s.to_string()
        } else if self.coerce {
            match value {
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => return Err(type_err(value)),
            }
        } else {
            return Err(type_err(value));
        };

        // Apply transforms
        for t in &self.transforms {
            match t {
                StringTransform::Trim => s = s.trim().to_string(),
                StringTransform::ToLowerCase => s = s.to_lowercase(),
                StringTransform::ToUpperCase => s = s.to_uppercase(),
            }
        }

        // Run checks, accumulate errors
        let str_val = Value::String(s.clone());
        let mut errors = VldError::new();

        for check in &self.checks {
            match check {
                StringCheck::Min(min, msg) => {
                    if s.chars().count() < *min {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: *min as f64,
                                inclusive: true,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Max(max, msg) => {
                    if s.chars().count() > *max {
                        errors.push_with_value(
                            IssueCode::TooBig {
                                maximum: *max as f64,
                                inclusive: true,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Len(len, msg) => {
                    if s.chars().count() != *len {
                        errors.push_with_value(
                            IssueCode::Custom {
                                code: "invalid_length".to_string(),
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Email(msg) => {
                    if !is_valid_email(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Email,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Url(msg) => {
                    if !is_valid_url(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Url,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Uuid(msg) => {
                    if !is_valid_uuid(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Uuid,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                #[cfg(feature = "regex")]
                StringCheck::Regex(re, msg) => {
                    if !re.is_match(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Regex,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::StartsWith(prefix, msg) => {
                    if !s.starts_with(prefix.as_str()) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::StartsWith,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::EndsWith(suffix, msg) => {
                    if !s.ends_with(suffix.as_str()) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::EndsWith,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Contains(sub, msg) => {
                    if !s.contains(sub.as_str()) {
                        errors.push_with_value(
                            IssueCode::Custom {
                                code: "invalid_string".to_string(),
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::NonEmpty(msg) => {
                    if s.is_empty() {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: 1.0,
                                inclusive: true,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Ipv4(msg) => {
                    if !is_valid_ipv4(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Ipv4,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Ipv6(msg) => {
                    if !is_valid_ipv6(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Ipv6,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Base64(msg) => {
                    if !is_valid_base64(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Base64,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::IsoDate(msg) => {
                    if !is_valid_iso_date(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::IsoDate,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::IsoDatetime(msg) => {
                    if !is_valid_iso_datetime(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::IsoDatetime,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::IsoTime(msg) => {
                    if !is_valid_iso_time(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::IsoTime,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Hostname(msg) => {
                    if !is_valid_hostname(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Hostname,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Cuid2(msg) => {
                    if !is_valid_cuid2(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Cuid2,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Ulid(msg) => {
                    if !is_valid_ulid(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Ulid,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Nanoid(msg) => {
                    if !is_valid_nanoid(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Nanoid,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
                StringCheck::Emoji(msg) => {
                    if !is_valid_emoji(&s) {
                        errors.push_with_value(
                            IssueCode::InvalidString {
                                validation: StringValidation::Emoji,
                            },
                            msg.clone(),
                            &str_val,
                        );
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(s)
        } else {
            Err(errors)
        }
    }
}
