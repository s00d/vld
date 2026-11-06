//! Text formats: hex, credit card, phone, slug, color, codes, cron, emoji.

pub(crate) fn is_valid_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn is_valid_credit_card(s: &str) -> bool {
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

pub(crate) fn is_valid_phone(s: &str) -> bool {
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
pub(crate) fn is_valid_phone_e164_strict(s: &str) -> bool {
    phonenumber::parse(None, s).is_ok_and(|n| phonenumber::is_valid(&n))
}

pub(crate) fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(crate) fn is_valid_color(s: &str) -> bool {
    if let Some(hex) = s.strip_prefix('#') {
        return (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    let lower = s.to_ascii_lowercase();
    // Plain digits only: `u8::from_str` would accept a leading `+`.
    let num = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let parts_of = |prefix: &str| -> Option<Vec<String>> {
        let inner = lower.strip_prefix(prefix)?.strip_suffix(')')?;
        let parts: Vec<String> = inner.split(',').map(|p| p.trim().to_string()).collect();
        (parts.len() == 3).then_some(parts)
    };
    if let Some(parts) = parts_of("rgb(") {
        return parts.iter().all(|p| num(p) && p.parse::<u8>().is_ok());
    }
    if let Some(parts) = parts_of("hsl(") {
        let pct = |p: &str| {
            p.strip_suffix('%')
                .filter(|v| num(v))
                .and_then(|v| v.parse::<u8>().ok())
                .is_some_and(|v| v <= 100)
        };
        return num(&parts[0])
            && parts[0].parse::<u16>().is_ok_and(|h| h <= 360)
            && pct(&parts[1])
            && pct(&parts[2]);
    }
    false
}

pub(crate) fn is_valid_currency_code(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase())
}

pub(crate) fn is_valid_country_code(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_uppercase())
}

pub(crate) fn is_valid_locale(s: &str) -> bool {
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

pub(crate) fn is_valid_cron(s: &str) -> bool {
    let fields: Vec<&str> = s.split_whitespace().collect();
    (fields.len() == 5 || fields.len() == 6)
        && fields.iter().all(|f| {
            f.bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'*' | b'/' | b',' | b'-' | b'?'))
        })
}

pub(crate) fn is_emoji_char(c: char) -> bool {
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

/// Joiners and modifiers that are not an emoji on their own: variation
/// selectors, ZWJ, the combining keycap and tag characters.
fn is_emoji_modifier(c: char) -> bool {
    matches!(c as u32, 0xFE00..=0xFE0F | 0x200D | 0x20E3 | 0xE0020..=0xE007F)
}

/// Made of emoji characters only (ZWJ / variation selectors allowed), with at
/// least one actual emoji (a lone U+FE0F or ZWJ is rejected).
pub(crate) fn is_valid_emoji(s: &str) -> bool {
    s.chars().all(is_emoji_char) && s.chars().any(|c| !is_emoji_modifier(c))
}
