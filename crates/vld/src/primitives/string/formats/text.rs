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

/// An active ISO 4217 code (the table `money()` uses).
pub(crate) fn is_valid_currency_code(s: &str) -> bool {
    crate::primitives::iso4217::is_currency(s)
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

/// Cron: 5 fields (`min hour dom month dow`) or 6 (seconds first), or one
/// of the `@yearly` / `@annually` / `@monthly` / `@weekly` / `@daily` /
/// `@midnight` / `@hourly` macros. A field is a `,` list of `*`, a value or
/// a range `a-b`, each with an optional `/step`; months and weekdays take
/// names (`JAN`, `mon`); `?` is allowed in the day fields; weekday `7` is
/// Sunday. Quartz extras (`L`, `W`, `#`) are rejected.
pub(crate) fn is_valid_cron(s: &str) -> bool {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    if s.starts_with('@') {
        return matches!(
            s,
            "@yearly" | "@annually" | "@monthly" | "@weekly" | "@daily" | "@midnight" | "@hourly"
        );
    }
    let fields: Vec<&str> = s.split_whitespace().collect();
    // (min, max, names (first = min), `?` allowed)
    let spec = [
        (0, 59, &[][..], false),
        (0, 59, &[][..], false),
        (0, 23, &[][..], false),
        (1, 31, &[][..], true),
        (1, 12, &MONTHS[..], false),
        (0, 7, &DAYS[..], true),
    ];
    let spec = match fields.len() {
        5 => &spec[1..],
        6 => &spec[..],
        _ => return false,
    };
    fields
        .iter()
        .zip(spec)
        .all(|(field, &(lo, hi, names, question))| {
            let value = |v: &str| -> Option<u32> {
                if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) {
                    return v.parse().ok().filter(|n| (lo..=hi).contains(n));
                }
                names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(v))
                    .map(|i| lo + i as u32)
            };
            (question && *field == "?")
                || field.split(',').all(|item| {
                    let (base, step) = match item.split_once('/') {
                        Some((b, st)) => (b, Some(st)),
                        None => (item, None),
                    };
                    let step_ok = step.is_none_or(|st| {
                        !st.is_empty()
                            && st.bytes().all(|b| b.is_ascii_digit())
                            && st
                                .parse::<u32>()
                                .is_ok_and(|n| (1..=hi.max(1)).contains(&n))
                    });
                    let base_ok = base == "*"
                        || match base.split_once('-') {
                            Some((a, b)) => {
                                matches!((value(a), value(b)), (Some(a), Some(b)) if a <= b)
                            }
                            None => value(base).is_some(),
                        };
                    step_ok && base_ok
                })
        })
}

/// RFC 6838 restricted name: 1-127 chars, alphanumeric first, then
/// alphanumerics and `!#$&-^_.+`.
fn is_mime_name(s: &str) -> bool {
    (1..=127).contains(&s.len())
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
}

/// Media type: `type/subtype[+suffix]` (RFC 6838 names) followed by
/// `; name=value` parameters (value: an RFC 9110 token or a quoted string).
pub(crate) fn is_valid_mime(s: &str) -> bool {
    let mut parts = s.split(';');
    let essence = parts.next().unwrap_or("");
    let Some((ty, sub)) = essence.split_once('/') else {
        return false;
    };
    let tchar = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
    let quoted = |v: &str| {
        let Some(inner) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
            return false;
        };
        let mut escaped = false;
        inner.chars().all(|c| {
            let ok = escaped || c != '"';
            escaped = !escaped && c == '\\';
            ok && !c.is_control()
        }) && !escaped
    };
    is_mime_name(ty)
        && is_mime_name(sub)
        && parts.all(|p| {
            let Some((name, value)) = p.trim_start_matches([' ', '\t']).split_once('=') else {
                return false;
            };
            is_mime_name(name) && (quoted(value) || (!value.is_empty() && value.bytes().all(tchar)))
        })
}

/// IANA time zone name. With the `jiff` feature: present in jiff's time
/// zone database, spelled as there (`Europe/Berlin`, not `europe/berlin`).
/// Without it: the shape of a name (`UTC`, `Etc/GMT+5`,
/// `America/Argentina/Buenos_Aires`): 1-3 `/`-separated parts, each starting
/// with an uppercase ASCII letter, then letters, digits, `_`, `-`, `+`.
pub(crate) fn is_valid_timezone(s: &str) -> bool {
    #[cfg(feature = "jiff")]
    {
        // jiff's lookup ignores ASCII case: the name must also be spelled
        // as in the database.
        is_timezone_shape(s)
            && jiff::tz::db()
                .get(s)
                .is_ok_and(|tz| tz.iana_name() == Some(s))
    }
    #[cfg(not(feature = "jiff"))]
    {
        is_timezone_shape(s)
    }
}

fn is_timezone_shape(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    s.len() <= 255
        && (1..=3).contains(&parts.len())
        && parts.iter().all(|p| {
            p.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+'))
        })
}

/// The value compiles as a regular expression (regex-lite syntax).
#[cfg(feature = "regex")]
pub(crate) fn is_valid_regex(s: &str) -> bool {
    regex_lite::Regex::new(s).is_ok()
}

/// Every character belongs to one script (Unicode TS #39 "single script",
/// with `Common` / `Inherited` characters and Han + Kana / Hangul mixes
/// allowed).
#[cfg(feature = "unicode")]
pub(crate) fn is_single_script(s: &str) -> bool {
    use unicode_security::MixedScript;
    s.is_single_script()
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
