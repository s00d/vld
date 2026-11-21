//! Stories: a public endpoint fed hostile input. Every failure is an error,
//! never a panic, a silent success, an SSRF target or a path outside the base.
//! Panic freedom on arbitrary input is `model_robustness`.

use crate::common::*;

use serde_json::json;
use vld::prelude::*;

/// Given a length-limited string field
/// When an attacker sends long text of multi-byte characters (so the 100-char
/// echo cut lands inside a character)
/// Then the error is `too_big` and the echoed value is a char-boundary
/// prefix of the input, at most 100 chars plus `...`.
#[test]
fn long_multibyte_text_is_echoed_on_char_boundaries() {
    let s = vld::string().max(10);
    for n in [51, 150] {
        let input = "é".repeat(n);
        let err = s.parse_value(&json!(input)).unwrap_err();
        assert_eq!(issues(&err), [(String::new(), "too_big".to_string())]);
        let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap();
        let kept = got.strip_suffix("...").unwrap_or(got);
        assert!(
            kept.chars().count() <= 100 && input.starts_with(kept),
            "{got:?}"
        );
    }
}

/// Given ISO date / time / datetime string checks (and the `time` backend)
/// When the input is cut short or carries non-ASCII where digits belong, or a
/// datetime lands beyond year 9999 after applying its offset
/// Then each is rejected without panicking, and a well-formed value passes.
#[test]
fn truncated_and_non_ascii_dates_are_rejected() {
    let date = vld::string().iso_date();
    for bad in ["123é56789", "2é24-01-01", "é"] {
        assert!(date.parse_value(&json!(bad)).is_err(), "{bad}");
    }
    assert!(
        vld::string()
            .iso_time()
            .parse_value(&json!("1é:00"))
            .is_err()
    );
    let dt = vld::string().iso_datetime();
    for bad in ["2024-01-01T", "2024-01-01Té", "2024-01-01Té:00:00"] {
        assert!(dt.parse_value(&json!(bad)).is_err(), "{bad}");
    }
    assert!(dt.parse_value(&json!("2024-01-01T10:00:00Z")).is_ok());

    #[cfg(feature = "time")]
    {
        let dt = vld::time::datetime();
        assert!(dt.parse_value(&json!("9999-12-31T23:00:00-05:00")).is_err());
        assert!(dt.parse_value(&json!("2024-01-01T10:00:00Z")).is_ok());
    }
}

/// Given an `i64` field with `multiple_of(-1)` (with and without `coerce`)
/// When the value saturates to the extreme of the integer range (`-1e40`, a
/// 39-digit negative string)
/// Then it is an error, not an overflow panic, and an ordinary value passes.
#[test]
fn extreme_numbers_do_not_overflow() {
    let s = vld::i64().multiple_of(-1);
    assert!(s.parse_value(&json!(-1e40)).is_err());
    let s = vld::i64().multiple_of(-1).coerce();
    assert!(
        s.parse_value(&json!("-170141183460469231731687303715884105728"))
            .is_err()
    );
    assert_eq!(s.parse_value(&json!(-7)).unwrap(), -7);
}

/// Given a webhook URL field with `url_strict()`
/// When the URL points at loopback, private, link-local, unspecified or
/// IPv4-mapped IPv6 addresses, `0.0.0.0` or a `localhost` name
/// Then it is rejected, and public hosts (name, IPv4, IPv6) pass.
#[cfg(feature = "string-advanced")]
#[test]
fn strict_urls_refuse_internal_hosts() {
    let s = vld::string().url_strict();
    for bad in [
        "http://[::1]/",
        "http://[fd00::1]/",
        "http://[fe80::1]/",
        "http://[::]/",
        "http://[::ffff:127.0.0.1]/",
        "http://[::ffff:10.0.0.1]/",
        "http://0.0.0.0/",
        "http://localhost./",
        "http://api.localhost/",
        "http://127.0.0.1/",
        "http://10.1.2.3/",
    ] {
        assert!(s.parse_value(&json!(bad)).is_err(), "{bad}");
    }
    for good in [
        "https://example.com/",
        "http://[2606:4700::1111]/",
        "http://8.8.8.8/",
    ] {
        assert!(s.parse_value(&json!(good)).is_ok(), "{good}");
    }
}

/// Given an upload path field confined with `path().within(base)`
/// When the path climbs out of the base through `..` to a file that does not
/// exist (absolute or relative)
/// Then it is rejected, and a new file inside the base passes.
#[test]
fn paths_cannot_escape_the_base() {
    let dir = tempdir();
    let base = dir.path();
    let schema = vld::path().within(base);
    let escape = format!("{}/../etc/passwd-does-not-exist", base.display());
    assert!(schema.parse_value(&json!(escape)).is_err());
    assert!(
        schema
            .parse_value(&json!("../outside-does-not-exist"))
            .is_err()
    );
    assert!(schema.parse_value(&json!("inside/new-file.txt")).is_ok());
}

/// A custom schema that fails without reporting any issue.
struct SilentFail;

impl VldSchema for SilentFail {
    type Output = ();
    fn parse_value(&self, _: &serde_json::Value) -> Result<(), VldError> {
        Err(VldError::new())
    }
}

#[cfg(feature = "openapi")]
impl vld::json_schema::JsonSchema for SilentFail {
    fn json_schema(&self) -> serde_json::Value {
        json!({})
    }
}

#[cfg(feature = "openapi")]
impl vld::json_schema::CollectNestedSchemas for SilentFail {}

vld::schema! {
    struct Silent {
        x: () => SilentFail,
    }
}

/// Given a user-written schema that fails with an empty issue list
/// When it sits inside every wrapper (intersection on either side, tuple,
/// array, set, map value, record, union, optional, refine, pipe, lazy) and as
/// a `schema!` field
/// Then every wrapper fails too, without panicking on `issues[0]`, and only
/// `catch` turns the failure into its fallback.
#[test]
fn issue_less_failures_are_never_swallowed() {
    let results = [
        (
            "intersection_first",
            vld::intersection(SilentFail, vld::any())
                .parse("1")
                .is_err(),
        ),
        (
            "intersection_second",
            vld::intersection(vld::any(), SilentFail)
                .parse("1")
                .is_err(),
        ),
        ("tuple", (SilentFail, vld::any()).parse("[1, 2]").is_err()),
        ("array", vld::array(SilentFail).parse("[1]").is_err()),
        ("set", vld::set(SilentFail).parse("[1]").is_err()),
        (
            "map",
            vld::map(vld::string(), SilentFail)
                .parse(r#"[["a", 1]]"#)
                .is_err(),
        ),
        (
            "record",
            vld::record(SilentFail).parse(r#"{"a": 1}"#).is_err(),
        ),
        (
            "union",
            vld::union(SilentFail, SilentFail).parse("1").is_err(),
        ),
        ("optional", SilentFail.optional().parse("1").is_err()),
        (
            "refine",
            SilentFail.refine(|_| true, "x").parse("1").is_err(),
        ),
        ("pipe", vld::any().pipe(SilentFail).parse("1").is_err()),
        ("lazy", vld::lazy(|| SilentFail).parse("1").is_err()),
        ("schema!", Silent::parse_value(&json!({"x": 1})).is_err()),
    ];
    let swallowed: Vec<&str> = results
        .into_iter()
        .filter(|(_, failed)| !failed)
        .map(|(name, _)| name)
        .collect();
    assert_eq!(swallowed, Vec::<&str>::new());
    assert!(SilentFail.catch(()).parse("1").is_ok());
}

/// Given a username field that normalizes to NFKC and rejects mixed scripts
/// When look-alike spellings arrive (decomposed accents, full-width letters,
/// a Cyrillic `а` inside a Latin name)
/// Then equal-looking names normalize to one stored form, and the
/// script-mixing spoof is one `single_script` issue.
#[cfg(feature = "unicode")]
#[test]
fn usernames_are_normalized_and_spoofs_rejected() {
    use vld::error::{IssueCode, StringValidation};
    let nfc = vld::string().normalize_nfc();
    assert_eq!(nfc.parse_value(&json!("Cafe\u{301}")).unwrap(), "Caf\u{e9}");
    // NFC keeps compatibility forms; NFKC folds them.
    assert_eq!(nfc.parse_value(&json!("ｆｉ")).unwrap(), "ｆｉ");
    let username = vld::string()
        .normalize_nfkc()
        .to_lowercase()
        .min(3)
        .max(4)
        .single_script();
    for spelling in ["ｃａｆｅ\u{301}", "CAFÉ", "cafe\u{301}", "ca\u{fb01}"] {
        let got = username.parse_value(&json!(spelling));
        match spelling {
            "ca\u{fb01}" => assert_eq!(got.unwrap(), "cafi"),
            _ => assert_eq!(got.unwrap(), "caf\u{e9}", "{spelling:?}"),
        }
    }
    // NFC also makes `max` count what the user sees (5 code points, 4 chars).
    assert!(
        vld::string()
            .max(4)
            .parse_value(&json!("cafe\u{301}"))
            .is_err()
    );
    let err = username.parse_value(&json!("p\u{430}yp")).unwrap_err();
    assert_eq!(
        err.issues.iter().map(|i| &i.code).collect::<Vec<_>>(),
        [&IssueCode::InvalidString {
            validation: StringValidation::SingleScript
        }]
    );
}
