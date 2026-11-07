//! Generate a random string matching a regular expression (JSON Schema `pattern`).

use crate::RngCompat;
use regex_syntax::hir::{Class, Hir, HirKind};

/// Upper bound for unbounded repetitions (`*`, `+`, `{n,}`): `min + EXTRA`.
const EXTRA: u32 = 8;

/// Random string matched by `pattern`, or `None` if it does not parse.
///
/// Anchors / word boundaries are ignored (they produce no text), so the
/// generated string is a full match of the pattern body.
pub(crate) fn generate<R: rand::Rng>(rng: &mut R, pattern: &str) -> Option<String> {
    let hir = regex_syntax::Parser::new().parse(pattern).ok()?;
    let mut out = String::new();
    walk(rng, &hir, &mut out);
    Some(out)
}

/// A pattern that is only a literal, anchored or not (what vld emits for
/// `starts_with` / `ends_with` / `contains`).
#[derive(Debug, PartialEq)]
pub(crate) enum Literal {
    Prefix(String),
    Suffix(String),
    Contains(String),
}

/// Classify `pattern` as a [`Literal`], or `None` for anything else.
pub(crate) fn literal(pattern: &str) -> Option<Literal> {
    let hir = regex_syntax::Parser::new().parse(pattern).ok()?;
    let parts: Vec<&Hir> = match hir.kind() {
        HirKind::Concat(subs) => subs.iter().collect(),
        _ => vec![&hir],
    };
    let (mut start, mut end, mut text) = (false, false, String::new());
    for (i, part) in parts.iter().enumerate() {
        match part.kind() {
            HirKind::Look(_) if i == 0 => start = true,
            HirKind::Look(_) if i == parts.len() - 1 => end = true,
            HirKind::Literal(lit) => text.push_str(std::str::from_utf8(&lit.0).ok()?),
            _ => return None,
        }
    }
    Some(match (start, end) {
        (true, false) => Literal::Prefix(text),
        (false, true) => Literal::Suffix(text),
        (false, false) => Literal::Contains(text),
        (true, true) => return None,
    })
}

fn walk<R: rand::Rng>(rng: &mut R, hir: &Hir, out: &mut String) {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => {}
        HirKind::Literal(lit) => out.push_str(&String::from_utf8_lossy(&lit.0)),
        HirKind::Class(Class::Unicode(cls)) => {
            let ranges: Vec<(u32, u32)> = cls
                .ranges()
                .iter()
                .map(|r| (r.start() as u32, r.end() as u32))
                .collect();
            if let Some(c) = pick_char(rng, &ranges) {
                out.push(c);
            }
        }
        HirKind::Class(Class::Bytes(cls)) => {
            let ranges: Vec<(u32, u32)> = cls
                .ranges()
                .iter()
                .map(|r| (r.start() as u32, r.end().min(0x7f) as u32))
                .filter(|(s, e)| s <= e)
                .collect();
            if let Some(c) = pick_char(rng, &ranges) {
                out.push(c);
            }
        }
        HirKind::Repetition(rep) => {
            let max = rep.max.unwrap_or(rep.min.saturating_add(EXTRA));
            let n = rng.gen_range(rep.min..=max.max(rep.min));
            for _ in 0..n {
                walk(rng, &rep.sub, out);
            }
        }
        HirKind::Capture(cap) => walk(rng, &cap.sub, out),
        HirKind::Concat(subs) => subs.iter().for_each(|h| walk(rng, h, out)),
        HirKind::Alternation(subs) => {
            if !subs.is_empty() {
                let i = rng.gen_range(0..subs.len());
                walk(rng, &subs[i], out);
            }
        }
    }
}

/// Pick a char from code-point ranges, preferring printable ASCII so `.` or
/// `\w` produce readable output.
fn pick_char<R: rand::Rng>(rng: &mut R, ranges: &[(u32, u32)]) -> Option<char> {
    let ascii: Vec<(u32, u32)> = ranges
        .iter()
        .map(|&(s, e)| (s.max(0x20), e.min(0x7e)))
        .filter(|(s, e)| s <= e)
        .collect();
    let pool = if ascii.is_empty() { ranges } else { &ascii };
    if pool.is_empty() {
        return None;
    }
    let (s, e) = pool[rng.gen_range(0..pool.len())];
    // Ranges from regex-syntax never contain surrogates; fall back to start.
    char::from_u32(rng.gen_range(s..=e)).or_else(|| char::from_u32(s))
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    #[test]
    fn matches_pattern() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        for p in [
            r"^[A-Z]{3}-\d{4}$",
            r"^(foo|bar)+baz?$",
            r"^\w+@\w+\.com$",
            r"^[^a-z]{2,5}$",
            r"^a/b\.c$",
        ] {
            let re = regex_lite::Regex::new(p).unwrap();
            for _ in 0..50 {
                let s = super::generate(&mut rng, p).unwrap();
                assert!(re.is_match(&s), "{p} vs {s:?}");
            }
        }
        assert!(super::generate(&mut rng, "(unclosed").is_none());
    }

    #[test]
    fn classifies_literals() {
        use super::{Literal::*, literal};
        assert_eq!(literal("^pre_"), Some(Prefix("pre_".into())));
        assert_eq!(literal("_suf$"), Some(Suffix("_suf".into())));
        assert_eq!(literal(r"a\.b"), Some(Contains("a.b".into())));
        assert_eq!(literal("^a+$"), None);
    }
}
