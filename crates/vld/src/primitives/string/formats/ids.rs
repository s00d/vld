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
    crate::primitives::bytes::decode_base64_url(s).is_some()
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
    // Crockford's Base32: 0-9 A-H J K M N P-T V-Z (no I, L, O, U); 26 chars
    // carry 130 bits, so the first must be 0-7 to fit the 128-bit value.
    s.len() == 26
        && matches!(s.as_bytes()[0], b'0'..=b'7')
        && s.chars().all(|c| {
            matches!(c.to_ascii_uppercase(), '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z')
        })
}

pub(crate) fn is_valid_nanoid(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// MongoDB ObjectId: 24 hex digits (either case).
pub(crate) fn is_valid_object_id(s: &str) -> bool {
    s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Digits of `s` with `-` / ` ` separators removed (`None` on any other
/// character, or a separator at either end or doubled).
fn digits_with_separators(s: &str) -> Option<Vec<u8>> {
    let sep = |b: u8| b == b'-' || b == b' ';
    let b = s.as_bytes();
    if b.first().is_some_and(|&c| sep(c))
        || b.last().is_some_and(|&c| sep(c))
        || b.windows(2).any(|w| sep(w[0]) && sep(w[1]))
    {
        return None;
    }
    Some(b.iter().copied().filter(|&c| !sep(c)).collect())
}

/// GS1 check digit (EAN-8/13, UPC-A, GTIN-14): weights 3,1,3,… from the
/// right, the last digit included, sum divisible by 10.
fn gs1_ok(d: &[u8]) -> bool {
    d.iter().all(u8::is_ascii_digit)
        && d.iter()
            .rev()
            .enumerate()
            .map(|(i, &c)| u32::from(c - b'0') * if i % 2 == 1 { 3 } else { 1 })
            .sum::<u32>()
            % 10
            == 0
}

/// EAN / GTIN: 8, 12, 13 or 14 digits (no separators) with a valid GS1
/// check digit.
pub(crate) fn is_valid_ean(s: &str) -> bool {
    matches!(s.len(), 8 | 12 | 13 | 14) && gs1_ok(s.as_bytes())
}

/// ISBN-10 (mod-11, check `X` = 10) or ISBN-13 (`978` / `979` + GS1 check
/// digit); single `-` or space separators allowed.
pub(crate) fn is_valid_isbn(s: &str) -> bool {
    let Some(d) = digits_with_separators(s) else {
        return false;
    };
    match d.len() {
        10 => {
            let (body, check) = d.split_at(9);
            let check = match check[0] {
                b'X' => 10,
                c if c.is_ascii_digit() => u32::from(c - b'0'),
                _ => return false,
            };
            body.iter().all(u8::is_ascii_digit)
                && (body
                    .iter()
                    .enumerate()
                    .map(|(i, &c)| (10 - i as u32) * u32::from(c - b'0'))
                    .sum::<u32>()
                    + check)
                    % 11
                    == 0
        }
        13 => (d.starts_with(b"978") || d.starts_with(b"979")) && gs1_ok(&d),
        _ => false,
    }
}

/// VIN (ISO 3779): 17 characters, digits and uppercase letters except
/// `I`, `O`, `Q`, with the North American check digit (49 CFR 565) in
/// position 9.
pub(crate) fn is_valid_vin(s: &str) -> bool {
    const WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'0'..=b'9' => u32::from(c - b'0'),
            b'A'..=b'H' => u32::from(c - b'A') + 1,
            b'J'..=b'N' => u32::from(c - b'J') + 1,
            b'P' => 7,
            b'R' => 9,
            b'S'..=b'Z' => u32::from(c - b'S') + 2,
            _ => return None,
        })
    };
    let b = s.as_bytes();
    if b.len() != 17 {
        return false;
    }
    let mut sum = 0;
    for (i, &c) in b.iter().enumerate() {
        let Some(v) = value(c) else {
            return false;
        };
        sum += v * WEIGHTS[i];
    }
    let check = match sum % 11 {
        10 => b'X',
        r => b'0' + r as u8,
    };
    b[8] == check
}
