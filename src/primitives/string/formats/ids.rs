//! Identifier / token formats: UUID, CUID2, ULID, Nano ID, JWT, Base64, semver.

pub(crate) fn is_valid_uuid(s: &str) -> bool {
    // 8-4-4-4-12 hex with dashes
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

#[cfg(feature = "string-advanced")]
pub(crate) fn is_valid_uuid_version(s: &str, version: usize) -> bool {
    uuid::Uuid::parse_str(s).is_ok_and(|u| u.get_version_num() == version)
}

pub(crate) fn is_valid_semver(s: &str) -> bool {
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
pub(crate) fn is_valid_semver_full(s: &str) -> bool {
    semver::Version::parse(s).is_ok()
}

pub(crate) fn is_valid_base64_url(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(crate) fn is_valid_jwt(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| is_valid_base64_url(p))
}

pub(crate) fn is_valid_base64(s: &str) -> bool {
    crate::primitives::bytes::decode_base64(s).is_some()
}

/// Base64URL (`-` / `_` alphabet), padding optional.
pub(crate) fn is_valid_base64url(s: &str) -> bool {
    !s.contains(['+', '/']) && crate::primitives::bytes::decode_base64_url(s).is_some()
}

/// A complete JSON document.
pub(crate) fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde::de::IgnoredAny>(s).is_ok()
}

pub(crate) fn is_valid_cuid2(s: &str) -> bool {
    (2..=32).contains(&s.len())
        && s.as_bytes()[0].is_ascii_lowercase()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

pub(crate) fn is_valid_ulid(s: &str) -> bool {
    // Crockford's Base32: 0-9 A-H J K M N P-T V-Z (no I, L, O, U)
    s.len() == 26
        && s.chars().all(|c| {
            matches!(c.to_ascii_uppercase(), '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z')
        })
}

pub(crate) fn is_valid_nanoid(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
