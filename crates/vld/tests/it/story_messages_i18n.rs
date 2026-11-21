//! Stories: what a user reads when validation fails — default English wording, custom messages
//! (`*_msg`, `type_error`, `with_messages`, `.message()`), reusable rules (`vld::rules`),
//! translation (`vld::i18n`, every shipped language), hand-built errors and error formatting
//! (`Display`, `vld::format`, `received`). Message texts are the subject here.
//!
//! Random string / number checks with `*_msg`, `with_messages` and `type_error` are the models in
//! `model_strings.rs` and `model_numbers.rs`; these stories pin the wording and the cases the
//! models do not generate (string formats, rules on other schemas, translation).

use crate::common::*;

use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use vld::error::{
    DateKind, FileTypeKind, FormatKind, IssueCode, PathSegment, SizeOrigin, StringValidation,
    ValidationIssue, VldError,
};
#[cfg(feature = "i18n")]
use vld::i18n::translator;
use vld::i18n::{
    FnResolver, MapResolver, MessageResolver, english, translate_error, translate_issue,
};
use vld::prelude::*;
use vld::rules::{Rule, from_fn};

fn messages(err: &VldError) -> Vec<&str> {
    err.issues.iter().map(|i| i.message.as_str()).collect()
}

fn first_msg<S: VldSchema>(s: &S, v: serde_json::Value) -> String
where
    S::Output: std::fmt::Debug,
{
    s.parse_value(&v).unwrap_err().issues[0].message.clone()
}

/// Given a sign-up form whose string fields carry per-check messages (`min_msg`, `email_msg`, … —
/// one for every string format) and a `type_error`
/// When each field gets a value that fails exactly its check
/// Then the issue keeps its code and shows exactly that message; the message never leaks onto
/// another check of the same schema, and `type_error` replaces only the type issue.
#[test]
fn per_check_messages_on_a_signup_form() {
    let s = vld::string;
    let cases: Vec<(ZString, &str, &str)> = vec![
        (s().min_msg(3, "m"), "ab", "too_small"),
        (s().max_msg(1, "m"), "ab", "too_big"),
        (s().len_msg(5, "m"), "ab", "invalid_length"),
        (s().non_empty_msg("m"), "", "too_small"),
        (s().starts_with_msg("x", "m"), "ab", "invalid_string"),
        (s().ends_with_msg("x", "m"), "ab", "invalid_string"),
        (s().contains_msg("x", "m"), "ab", "invalid_string"),
        (s().email_msg("m"), "nope", "invalid_string"),
        (s().url_msg("m"), "nope", "invalid_string"),
        (s().uuid_msg("m"), "nope", "invalid_string"),
        (s().ipv4_msg("m"), "nope", "invalid_string"),
        (s().ipv6_msg("m"), "nope", "invalid_string"),
        (s().base64_msg("m"), "!!", "invalid_string"),
        (s().iso_date_msg("m"), "nope", "invalid_string"),
        (s().iso_time_msg("m"), "nope", "invalid_string"),
        (s().iso_datetime_msg("m"), "nope", "invalid_string"),
        (s().hostname_msg("m"), "-nope", "invalid_string"),
        (s().cuid2_msg("m"), "!!", "invalid_string"),
        (s().ulid_msg("m"), "short", "invalid_string"),
        (s().nanoid_msg("m"), "hello world", "invalid_string"),
        (s().emoji_msg("m"), "plain", "invalid_string"),
        (s().slug_msg("m"), "No Slug", "invalid_string"),
        (s().cron_msg("m"), "x", "invalid_string"),
        (s().base64url_msg("m"), "+", "invalid_string"),
    ];
    for (schema, input, code) in cases {
        let err = schema.parse_value(&json!(input)).unwrap_err();
        assert_eq!(
            issues(&err),
            [(String::new(), code.to_string())],
            "{input:?}"
        );
        assert_eq!(messages(&err), ["m"], "{input:?}");
    }

    #[cfg(feature = "regex")]
    assert_eq!(
        first_msg(&s().pattern_msg("^a+$", "only a"), json!("b")),
        "only a"
    );
    #[cfg(feature = "string-advanced")]
    {
        assert_eq!(
            first_msg(
                &s().uuid_v1_msg("v1 only"),
                json!("550e8400-e29b-41d4-a716-446655440000")
            ),
            "v1 only"
        );
        assert_eq!(
            first_msg(&s().url_strict_msg("public"), json!("http://10.0.0.1/")),
            "public"
        );
    }

    // A per-check message belongs to its check only.
    let err = s()
        .min_msg(3, "At least 3 chars")
        .email()
        .parse_value(&json!("ab"))
        .unwrap_err();
    assert_eq!(messages(&err)[0], "At least 3 chars");
    assert_ne!(messages(&err)[1], "At least 3 chars");

    // type_error replaces the type issue only.
    let schema = s().min(5).type_error("Must be text!");
    assert_eq!(first_msg(&schema, json!(42)), "Must be text!");
    assert_ne!(first_msg(&schema, json!("ab")), "Must be text!");
}

/// Given schemas with bulk messages: `with_messages` by key (before or after the checks it
/// covers) and `.message()` over a whole schema
/// When values fail several checks
/// Then answered keys get the bulk text, unanswered keys keep the default, a per-check message
/// beats the bulk closure, `.message()` replaces every issue message, and codes never change.
#[test]
fn bulk_messages_by_key_and_for_a_whole_schema() {
    let schema = vld::string()
        .min(5)
        .email()
        .url()
        .with_messages(|key| match key {
            "too_small" => Some("Too short!".into()),
            "invalid_email" => Some("Bad email!".into()),
            _ => None,
        });
    let err = schema.parse_value(&json!("hi")).unwrap_err();
    assert_eq!(messages(&err)[..2], ["Too short!", "Bad email!"]);
    assert!(messages(&err)[2].contains("URL"), "{}", messages(&err)[2]);
    assert_eq!(
        issues(&err),
        [
            (String::new(), "too_small".into()),
            (String::new(), "invalid_string".into()),
            (String::new(), "invalid_string".into())
        ]
    );
    let json_schema = vld::string()
        .json()
        .with_messages(|k| (k == "invalid_json").then(|| "json!".into()));
    assert_eq!(first_msg(&json_schema, json!("{")), "json!");

    // Checks added after with_messages are covered too.
    let s = vld::string()
        .with_messages(|k| (k == "too_small").then(|| "short!".to_string()))
        .min(3);
    assert_eq!(first_msg(&s, json!("ab")), "short!");
    let n = vld::number()
        .with_messages(|k| (k == "too_big").then(|| "big!".to_string()))
        .max(1.0);
    assert_eq!(first_msg(&n, json!(2)), "big!");
    // A per-check message wins over the bulk closure.
    let s = vld::string()
        .min_msg(3, "own")
        .with_messages(|_| Some("bulk".into()));
    assert_eq!(first_msg(&s, json!("ab")), "own");

    // `.message()` replaces every message of the wrapped schema.
    let s = vld::number().int().min(0).max(100).message("Bad number");
    let err = s.parse_value(&json!("not a number")).unwrap_err();
    assert_eq!(messages(&err), ["Bad number"]);
    assert_eq!(issues(&err), [(String::new(), "invalid_type".into())]);
    let err = vld::string()
        .email()
        .message("Invalid email")
        .parse("\"x\"")
        .unwrap_err();
    assert_eq!(messages(&err), ["Invalid email"]);
    assert_eq!(issues(&err), [(String::new(), "invalid_string".into())]);
    assert_eq!(
        vld::string()
            .min(2)
            .message("e")
            .parse(r#""hello""#)
            .unwrap(),
        "hello"
    );
}

/// Given schemas without any custom message
/// When they fail
/// Then the default English wording is used: type labels, `_one` singular forms, number wording
/// for integer bounds of one, "greater than" for exclusive bounds, and the `english()` resolver
/// reproduces every default message.
#[test]
fn default_english_wording() {
    let cases: Vec<(String, &str)> = vec![
        (
            first_msg(&vld::string(), json!(42)),
            "Expected string, received number",
        ),
        (
            first_msg(&vld::string(), json!(null)),
            "Expected string, received null",
        ),
        (
            first_msg(&vld::string().min(5), json!("ab")),
            "String must be at least 5 characters",
        ),
        (
            first_msg(&vld::string().base64url(), json!("+")),
            "Invalid Base64URL string",
        ),
        (
            first_msg(&vld::string().json(), json!("{")),
            "Invalid JSON string",
        ),
        (
            first_msg(&vld::i32().coerce(), json!("1.5")),
            "Expected integer",
        ),
        (
            first_msg(&vld::char(), json!("ab")),
            "String must be exactly 1 character",
        ),
        (
            first_msg(&vld::array(vld::string()).min_len(1), json!([])),
            "Array must have at least 1 element",
        ),
        (
            first_msg(&vld::u16().min(1), json!(0)),
            "Number must be at least 1",
        ),
        (
            first_msg(&vld::i64().max(1), json!(5)),
            "Number must be at most 1",
        ),
        (
            first_msg(&vld::number().positive(), json!(0)),
            "Number must be greater than 0",
        ),
        (
            first_msg(&vld::number().min(3.0), json!(1)),
            "Number must be at least 3",
        ),
        (
            first_msg(&vld::path(), json!(1)),
            "Expected path string, received number",
        ),
    ];
    for (got, want) in cases {
        assert_eq!(got, want);
    }
    assert!(first_msg(&vld::duration(), json!(true)).starts_with("Expected duration as seconds"));
    let err = vld::duration().parse_value(&json!(true)).unwrap_err();
    assert!(
        matches!(&err.issues[0].code, IssueCode::InvalidType { expected, .. } if expected == "duration")
    );
    let err = vld::path().parse_value(&json!(1)).unwrap_err();
    assert!(
        matches!(&err.issues[0].code, IssueCode::InvalidType { expected, .. } if expected == "path")
    );

    // english() is exactly the default table.
    let errs = [
        vld::string().min(3).parse(r#""a""#).unwrap_err(),
        vld::number().max(3.0).parse("4").unwrap_err(),
        vld::number().int().parse("1.5").unwrap_err(),
        vld::array(vld::any()).unique().parse("[1, 1]").unwrap_err(),
        vld::number().positive().parse("0").unwrap_err(),
    ];
    for err in errs {
        let issue = &err.issues[0];
        assert!(!issue.has_custom_message());
        assert_eq!(translate_issue(issue, &english()).message, issue.message);
    }
}

/// Given an app that ships its own translations as a `MapResolver` or an `FnResolver`
/// When it translates a validation error
/// Then templates are filled from the code's params, the most specific key wins
/// (`too_small.string` over `too_small`), exclusive bounds use `*_exclusive` keys, unknown keys
/// keep the original message, custom messages are never translated, and code and path survive.
#[test]
fn translating_errors_with_own_resolvers() {
    let err = vld::string().min(5).parse(r#""ab""#).unwrap_err();
    let map = MapResolver::new(HashMap::from([(
        "too_small".into(),
        "Минимум {minimum}".into(),
    )]));
    assert_eq!(translate_error(&err, &map).issues[0].message, "Минимум 5");
    let f = FnResolver::new(|key| (key == "too_small").then(|| "Zu kurz! Min: {minimum}".into()));
    let tr = translate_error(&err, &f);
    assert_eq!(tr.issues[0].message, "Zu kurz! Min: 5");
    assert_eq!(tr.issues[0].code, err.issues[0].code);
    assert_eq!(tr.issues[0].path, err.issues[0].path);

    let r = FnResolver::new(|k| match k {
        "too_small" => Some("generic".into()),
        "too_small.string" => Some("string-specific {minimum}".into()),
        _ => None,
    });
    let err = vld::string().min(2).parse(r#""a""#).unwrap_err();
    assert_eq!(
        translate_error(&err, &r).issues[0].message,
        "string-specific 2"
    );
    let err = vld::number().min(2.0).parse("1").unwrap_err();
    assert_eq!(translate_error(&err, &r).issues[0].message, "generic");

    let err = vld::string().email().parse(r#""bad""#).unwrap_err();
    assert_eq!(
        translate_error(&err, &FnResolver::new(|_| None)).issues[0].message,
        err.issues[0].message
    );

    let generic = FnResolver::new(|k| match k {
        "too_small" => Some("at least {minimum}".into()),
        "too_small_exclusive" => Some("more than {minimum}".into()),
        _ => None,
    });
    let err = vld::number().positive().parse("0").unwrap_err();
    assert_eq!(
        translate_error(&err, &generic).issues[0].message,
        "more than 0"
    );

    // Custom messages (per-check or bulk) are kept by translation.
    let err = vld::string()
        .min_msg(3, "mine")
        .parse(r#""ab""#)
        .unwrap_err();
    assert!(err.issues[0].has_custom_message());
    assert_eq!(translate_error(&err, &generic).issues[0].message, "mine");

    // A placeholder the code has no parameter for is kept as written.
    let r = FnResolver::new(|k| (k == "too_small").then(|| "Bad {unknown}: {minimum}!".into()));
    let err = vld::string().min(5).parse(r#""ab""#).unwrap_err();
    assert_eq!(
        translate_error(&err, &r).issues[0].message,
        "Bad {unknown}: 5!"
    );

    // Count variants: a resolver that only implements `resolve` uses the English plural rule,
    // like `MapResolver::new`.
    struct OwnTable;
    impl MessageResolver for OwnTable {
        fn resolve(&self, key: &str) -> Option<String> {
            match key {
                "too_small.string_one" => Some("one char".into()),
                "too_small.string" => Some("{minimum} chars".into()),
                _ => None,
            }
        }
    }
    let map = MapResolver::new(HashMap::from([
        ("too_small.string_one".into(), "one char".into()),
        ("too_small.string".into(), "{minimum} chars".into()),
    ]));
    let one = vld::string().min(1).parse(r#""""#).unwrap_err();
    let two = vld::string().min(2).parse(r#""a""#).unwrap_err();
    for r in [&OwnTable as &dyn MessageResolver, &map] {
        assert_eq!(translate_error(&one, r).issues[0].message, "one char");
        assert_eq!(translate_error(&two, r).issues[0].message, "2 chars");
    }
    assert_eq!(
        [0, 1, 2, 21].map(vld::i18n::plural_en),
        ["other", "one", "other", "other"]
    );
}

/// Given an error translated into each shipped language (the embedded catalogs, feature `i18n`)
/// When the user reads it
/// Then the text differs from English but keeps the params, type names are translated,
/// Russian uses its plural forms, custom messages stay, a union's `[variant N]` prefix is
/// translated, and object issues (`unrecognized_field`, `missing_field`) are translated too.
#[cfg(feature = "i18n")]
#[test]
fn shipped_languages() {
    let err = vld::number().min(10.0).parse("5").unwrap_err();
    let en = translate_error(&err, &english()).issues[0].message.clone();
    for lang in vld::i18n::locales().filter(|l| *l != "en").map(translator) {
        let msg = translate_error(&err, &lang).issues[0].message.clone();
        assert!(msg.contains("10"), "{msg}");
        assert_ne!(msg, en);
        let lit = vld::literal("a").parse_value(&json!("b")).unwrap_err();
        let msg = translate_error(&lit, &lang).issues[0].message.clone();
        assert!(!msg.starts_with("Expected"), "{msg}");
    }

    let ru = |s: &ZString, v: serde_json::Value| {
        translate_error(&s.parse_value(&v).unwrap_err(), &translator("ru")).issues[0]
            .message
            .clone()
    };
    assert_eq!(
        ru(&vld::string(), json!(true)),
        "Ожидался тип «строка», получен «логическое значение»"
    );
    assert!(ru(&vld::string().min(3), json!("ab")).contains("3 символов"));
    assert_eq!(
        ru(&vld::string().len(1), json!("")),
        "Строка должна содержать ровно 1 символ"
    );
    assert_eq!(
        ru(&vld::string().len(3), json!("")),
        "Строка должна содержать ровно 3 символа"
    );
    assert_eq!(
        ru(&vld::string().len(5), json!("")),
        "Строка должна содержать ровно 5 символов"
    );
    assert_eq!(
        ru(&vld::string().len(12), json!("")),
        "Строка должна содержать ровно 12 символов"
    );
    assert_eq!(
        ru(&vld::string().min(21), json!("")),
        "Строка должна содержать не менее 21 символа"
    );
    let err = vld::duration().parse_value(&json!(true)).unwrap_err();
    assert_eq!(
        translate_error(&err, &translator("ru")).issues[0].message,
        "Ожидался тип «длительность в секундах или строкой (`PT15S`, `1.5s`, `250ms`)», получен «логическое значение»"
    );

    // Union prefix: translated in Russian, kept with a translated message in German.
    let err = vld::union(vld::string(), vld::number())
        .parse_value(&json!(true))
        .unwrap_err();
    let tr = translate_error(&err, &translator("ru"));
    assert!(
        tr.issues
            .iter()
            .any(|i| i.message.starts_with("[вариант 1] ")),
        "{:?}",
        messages(&tr)
    );
    assert!(!tr.issues.iter().any(|i| i.message.contains("[variant")));
    let u = vld::union(vld::string().min(3), vld::number());
    let tr = translate_error(&u.parse_value(&json!("ab")).unwrap_err(), &translator("de"));
    assert_eq!(
        tr.issues[1].message,
        "[Variante 1] Zeichenkette muss mindestens 3 Zeichen lang sein"
    );

    // German keeps custom and bulk messages, translates defaults.
    let de = |e: &VldError| {
        translate_error(e, &translator("de")).issues[0]
            .message
            .clone()
    };
    assert_eq!(
        de(&vld::string()
            .min_msg(3, "my custom")
            .parse(r#""ab""#)
            .unwrap_err()),
        "my custom"
    );
    let bulk = vld::string().min(3).with_messages(|_| Some("bulk".into()));
    assert_eq!(de(&bulk.parse(r#""ab""#).unwrap_err()), "bulk");
    assert_eq!(
        de(&vld::string().min(3).parse(r#""ab""#).unwrap_err()),
        "Zeichenkette muss mindestens 3 Zeichen lang sein"
    );
    let strict = vld::object().field("a", vld::string()).strict();
    let err = strict.parse_value(&json!({"a": "x", "b": 1})).unwrap_err();
    assert_eq!(issues(&err), [(".b".into(), "unrecognized_field".into())]);
    assert_eq!(de(&err), "Unbekanntes Feld");
    let req = vld::object().field("a", vld::string()).partial().required();
    assert_eq!(
        de(&req.parse_value(&json!({"a": null})).unwrap_err()),
        "Pflichtfeld fehlt"
    );
}

/// One code of every kind vld produces.
fn sample_codes() -> Vec<IssueCode> {
    use IssueCode::*;
    vec![
        InvalidType {
            expected: "string".into(),
            received: "number".into(),
        },
        TooSmall {
            origin: SizeOrigin::String,
            minimum: json!(1),
            inclusive: true,
        },
        TooBig {
            origin: SizeOrigin::Number,
            maximum: json!(1),
            inclusive: false,
        },
        InvalidLength {
            origin: SizeOrigin::Array,
            expected: 2,
        },
        InvalidString {
            validation: StringValidation::StartsWith { prefix: "a".into() },
        },
        InvalidFormat {
            format: FormatKind::Hex,
        },
        InvalidDate {
            kind: DateKind::Date,
        },
        InvalidTimezoneOffset {
            expected: "+03:00".into(),
        },
        NotInPast {
            kind: DateKind::Date,
        },
        NotInFuture {
            kind: DateKind::DateTime,
        },
        NotInt,
        NotFinite,
        NotMultipleOf { divisor: json!(2) },
        NotSafeInteger,
        InvalidLiteral { expected: json!(1) },
        InvalidEnumValue {
            options: vec!["a".into()],
        },
        InvalidUnion,
        InvalidDiscriminator {
            options: vec![json!("a")],
        },
        NotUnique,
        MissingContains,
        DuplicateKey,
        InvalidMapEntry,
        RingNotClosed,
        RangeOrder {
            start: "from".into(),
            end: "to".into(),
        },
        MissingField,
        UnrecognizedField,
        PathOutsideBase { base: "/b".into() },
        PathNotAbsolute,
        PathNotRelative,
        PathNotFound,
        PathNotFile,
        PathNotDir,
        JsonTooDeep { max_depth: 1 },
        FileTypeNotAllowed {
            kind: FileTypeKind::Extension,
            actual: "exe".into(),
            allowed: vec!["png".into()],
        },
        FileTypeDenied {
            kind: FileTypeKind::MediaType,
            actual: "x".into(),
        },
        FileTypeUnknown {
            kind: FileTypeKind::MagicType,
        },
        InvalidCryptoAddress {
            network: "bitcoin".into(),
            reason: vld::error::CryptoAddressReason::Checksum,
        },
        ChecksumMismatch {
            algorithm: "MD5".into(),
        },
        MissingExif,
        SerializeError { detail: "x".into() },
        IoError,
        ParseError,
        Custom {
            code: "mine".into(),
        },
    ]
}

fn placeholders(s: &str) -> BTreeSet<String> {
    s.split('{')
        .skip(1)
        .filter_map(|p| p.split_once('}').map(|(n, _)| n.to_string()))
        .collect()
}

/// Every template key a code family can ask for: `<base>.<sub>` for every sub-kind, with and
/// without the `_one` plural form, plus the type labels.
#[cfg(feature = "i18n")]
fn candidate_keys() -> Vec<String> {
    use StringValidation::*;
    let origins = [
        SizeOrigin::String,
        SizeOrigin::Number,
        SizeOrigin::Decimal,
        SizeOrigin::Array,
        SizeOrigin::Tuple,
        SizeOrigin::Set,
        SizeOrigin::Record,
        SizeOrigin::Object,
        SizeOrigin::Bytes,
        SizeOrigin::Date,
        SizeOrigin::DateTime,
        SizeOrigin::Time,
        SizeOrigin::Duration,
        SizeOrigin::File,
        SizeOrigin::ImageWidth,
        SizeOrigin::ImageHeight,
        SizeOrigin::Port,
        SizeOrigin::Contains,
        SizeOrigin::Scale,
        SizeOrigin::Precision,
    ];
    let validations = [
        Email,
        Url,
        UrlStrict,
        Uri,
        Uuid,
        UuidV1,
        UuidV4,
        UuidV7,
        Ip,
        Slug,
        Color,
        Currency,
        CountryCode,
        Locale,
        Cron,
        Regex,
        StartsWith { prefix: "p".into() },
        EndsWith { suffix: "s".into() },
        Contains {
            substring: "c".into(),
        },
        Ipv4,
        Ipv6,
        Cidr,
        Mac,
        Hex,
        CreditCard,
        Phone,
        PhoneE164,
        Semver,
        SemverFull,
        Jwt,
        Ascii,
        Alpha,
        Alphanumeric,
        Lowercase,
        Uppercase,
        Base64,
        Base64Url,
        Json,
        IsoDate,
        IsoDatetime,
        IsoTime,
        Hostname,
        Cuid2,
        Ulid,
        Nanoid,
        Emoji,
        Country,
        CountryAlpha3,
        Iban,
        Bic,
        Isbn,
        Ean,
        Vin,
        Timezone,
        Mime,
        ObjectId,
        RegexSyntax,
        SingleScript,
        PasswordLowercase,
        PasswordUppercase,
        PasswordDigit,
        PasswordSymbol,
        PasswordRepeat { max_repeat: 2 },
        PasswordStrength { min_score: 3 },
    ];
    let formats = [
        FormatKind::Decimal,
        FormatKind::IpNetwork,
        FormatKind::Ipv4Network,
        FormatKind::Ipv6Network,
        FormatKind::SocketAddr,
        FormatKind::Base64,
        FormatKind::Base64Url,
        FormatKind::Hex,
        FormatKind::Image,
        FormatKind::FromStr,
        FormatKind::DurationCalendarUnit,
    ];
    let mut keys: Vec<String> = sample_codes()
        .iter()
        .flat_map(|c| c.message_keys())
        .collect();
    keys.extend(
        [
            "too_small",
            "too_big",
            "too_small_exclusive",
            "too_big_exclusive",
            "custom",
        ]
        .map(String::from),
    );
    for base in [
        "too_small",
        "too_big",
        "too_small_exclusive",
        "too_big_exclusive",
        "invalid_length",
    ] {
        keys.extend(origins.iter().map(|o| format!("{base}.{}", o.key())));
    }
    keys.extend(
        validations
            .iter()
            .map(|v| format!("invalid_string.{}", v.key())),
    );
    keys.extend(
        formats
            .iter()
            .map(|f| format!("invalid_format.{}", f.key())),
    );
    for base in ["invalid_date", "not_in_past", "not_in_future"] {
        for k in [DateKind::Date, DateKind::DateTime, DateKind::Time] {
            keys.push(format!("{base}.{}", k.key()));
        }
    }
    let ones: Vec<String> = keys.iter().map(|k| format!("{k}_one")).collect();
    keys.extend(ones);
    for t in [
        "string",
        "number",
        "integer",
        "boolean",
        "array",
        "object",
        "null",
        "never",
        "geo_point",
        "date",
        "datetime",
        "time",
        "duration",
        "decimal",
        "path",
        "file",
        "socket_addr",
        "ip_network",
        "bytes",
        "bytes_or_string",
        "byte",
    ] {
        keys.push(format!("type.{t}"));
    }
    keys
}

/// Given every kind of issue code vld can produce
/// When its message is rendered in English or any shipped language
/// Then every code has a generic English template, every size origin has `too_small` /
/// `too_big` templates, templates only use params the code provides (no `{…}` left in the
/// default message or the English translation), and Russian, German and Spanish answer every
/// English key with the same placeholders.
#[test]
fn every_code_has_a_template_in_every_language() {
    let en = english();
    for code in sample_codes() {
        let keys = code.message_keys();
        assert!(
            en.resolve(keys.last().unwrap()).is_some(),
            "no English template for {keys:?}"
        );
        let params: BTreeSet<String> = code
            .params()
            .into_iter()
            .map(|(n, _)| n.to_string())
            .collect();
        for key in &keys {
            for k in [key.clone(), format!("{key}_one")] {
                if let Some(t) = en.resolve(&k) {
                    assert!(
                        placeholders(&t).is_subset(&params),
                        "`{k}`: {t} vs {params:?}"
                    );
                }
            }
        }
        assert!(
            !code.default_message().contains('{'),
            "{}",
            code.default_message()
        );
        let issue = ValidationIssue::new(code.clone());
        let tr = translate_issue(&issue, &en).message;
        assert!(!tr.contains('{'), "{tr}");
    }
    for o in [
        SizeOrigin::String,
        SizeOrigin::Number,
        SizeOrigin::Decimal,
        SizeOrigin::Array,
        SizeOrigin::Set,
        SizeOrigin::Record,
        SizeOrigin::Object,
        SizeOrigin::Bytes,
        SizeOrigin::Date,
        SizeOrigin::DateTime,
        SizeOrigin::Time,
        SizeOrigin::Duration,
        SizeOrigin::File,
        SizeOrigin::ImageWidth,
        SizeOrigin::ImageHeight,
        SizeOrigin::Port,
        SizeOrigin::Contains,
    ] {
        for base in ["too_small", "too_big"] {
            assert!(
                en.resolve(&format!("{base}.{}", o.key())).is_some(),
                "{base}.{}",
                o.key()
            );
        }
    }

    #[cfg(feature = "i18n")]
    {
        // Every language answers every English key, with English's placeholders.
        let keys = candidate_keys();
        for locale in vld::i18n::locales() {
            let lang = translator(locale);
            let own = catalog_keys(locale);
            for key in &keys {
                let Some(t_en) = en.resolve(key).filter(|_| !key.ends_with("_one")) else {
                    continue;
                };
                assert!(
                    own.contains(&format!("vld.{key}")),
                    "{locale}: missing `{key}`"
                );
                let t = lang.resolve(key).unwrap();
                assert_eq!(placeholders(&t), placeholders(&t_en), "{locale}: `{key}`");
            }
            assert!(own.contains("vld.union_variant"), "{locale}: union_variant");
        }
    }
}

/// `+7` and ten digits.
struct RuPhone;

impl Rule<str> for RuPhone {
    fn key(&self) -> &str {
        "ru_phone"
    }
    fn check(&self, s: &str) -> bool {
        s.strip_prefix("+7")
            .is_some_and(|d| d.len() == 10 && d.bytes().all(|b| b.is_ascii_digit()))
    }
    #[cfg(feature = "openapi")]
    fn json_schema(&self) -> Option<serde_json::Value> {
        Some(json!({ "format": "ru-phone" }))
    }
}

/// A rule with its own default message.
struct NoSpaces;

impl Rule<str> for NoSpaces {
    fn key(&self) -> &str {
        "no_spaces"
    }
    fn check(&self, s: &str) -> bool {
        !s.contains(' ')
    }
    fn message(&self) -> Option<String> {
        Some("Spaces are not allowed".into())
    }
}

fn keys(e: &VldError) -> Vec<String> {
    e.issues.iter().map(|i| i.code.key().to_string()).collect()
}

/// Given a team's reusable rules (`impl Rule<str>`, `rules::from_fn`, shared through `Arc`)
/// attached to strings, integers, arrays and a derived struct
/// When values break them
/// Then each rule reports a `Custom` code under its key in order with built-in checks (after the
/// inner schema passes), with message precedence `rule_msg` > `with_messages` > the rule's own
/// message > `"Validation error: <key>"`, translation by the rule key (the rule's own message is
/// kept), and the rule's JSON Schema keywords merged into the schema (clashes go to `allOf`).
#[test]
fn reusable_rules_across_schemas() {
    let s = vld::string().trim().rule(RuPhone).min(5);
    assert_eq!(s.parse(r#"" +79991234567 ""#).unwrap(), "+79991234567");
    let e = s.parse(r#""+7""#).unwrap_err();
    assert_eq!(keys(&e), ["ru_phone", "too_small"]);
    assert_eq!(
        e.issues[0].code,
        IssueCode::Custom {
            code: "ru_phone".into()
        }
    );
    assert_eq!(e.issues[0].received, Some(json!("+7")));

    // Message precedence.
    let e = vld::string().rule(RuPhone).parse(r#""x""#).unwrap_err();
    assert_eq!(e.issues[0].message, "Validation error: ru_phone");
    let e = vld::string().rule(NoSpaces).parse(r#""a b""#).unwrap_err();
    assert_eq!(e.issues[0].message, "Spaces are not allowed");
    let e = vld::string()
        .rule(NoSpaces)
        .with_messages(|k| (k == "no_spaces").then(|| "no spaces!".to_string()))
        .parse(r#""a b""#)
        .unwrap_err();
    assert_eq!(e.issues[0].message, "no spaces!");
    let e = vld::string()
        .rule_msg(NoSpaces, "custom text")
        .with_messages(|_| Some("bulk".to_string()))
        .parse(r#""a b""#)
        .unwrap_err();
    assert_eq!(e.issues[0].message, "custom text");

    // Translation by key.
    let resolver = MapResolver::new(HashMap::from([(
        "ru_phone".to_string(),
        "Нужен номер +7XXXXXXXXXX".to_string(),
    )]));
    let e = vld::string().rule(RuPhone).parse(r#""8""#).unwrap_err();
    assert_eq!(
        translate_error(&e, &resolver).issues[0].message,
        "Нужен номер +7XXXXXXXXXX"
    );
    let e = vld::string().rule(NoSpaces).parse(r#""a b""#).unwrap_err();
    assert_eq!(
        translate_error(&e, &resolver).issues[0].message,
        "Spaces are not allowed"
    );

    // Rules on any schema run after it.
    let even = from_fn("even", |n: &i64| n % 2 == 0).message("Must be even");
    let s = vld::number().int().min(0).rule(even);
    assert_eq!(s.parse("4").unwrap(), 4);
    let e = s.parse("3").unwrap_err();
    assert_eq!(keys(&e), ["even"]);
    assert_eq!(e.issues[0].message, "Must be even");
    assert_eq!(keys(&s.parse("-1").unwrap_err()), ["too_small"]);
    let distinct = from_fn("distinct", |v: &Vec<i64>| {
        let mut s = v.clone();
        s.sort_unstable();
        s.windows(2).all(|w| w[0] != w[1])
    });
    let list = vld::array(vld::number().int()).rule_msg(distinct, "duplicates");
    assert!(list.parse("[1,2,3]").is_ok());
    assert_eq!(
        list.parse("[1,2,1]").unwrap_err().issues[0].message,
        "duplicates"
    );

    // A rule shared between schemas.
    let shared: Arc<dyn Rule<str>> = Arc::new(from_fn("lower", |s: &str| {
        s.chars().all(|c| !c.is_uppercase())
    }));
    let a = vld::string().rule(shared.clone());
    let b = vld::string().min(1).rule(shared);
    assert!(a.parse(r#""abc""#).is_ok());
    assert_eq!(keys(&b.parse(r#""aBc""#).unwrap_err()), ["lower"]);
    // A shared rule keeps its own message and JSON Schema keywords, on any schema.
    let shared_even: Arc<dyn Rule<i64>> = Arc::new({
        let r = from_fn("even", |n: &i64| n % 2 == 0).message("Must be even");
        #[cfg(feature = "openapi")]
        let r = r.json_schema(json!({ "multipleOf": 2 }));
        r
    });
    let n = vld::number().int().rule(shared_even.clone());
    assert_eq!(messages(&n.parse("3").unwrap_err()), ["Must be even"]);
    #[cfg(feature = "openapi")]
    {
        use vld::json_schema::JsonSchema;
        assert_eq!(n.json_schema(), json!({"type": "integer", "multipleOf": 2}));
    }

    #[cfg(feature = "derive")]
    {
        #[derive(Debug, vld::Validate)]
        #[allow(dead_code)]
        struct Contact {
            #[vld(vld::string().rule(RuPhone))]
            phone: String,
        }
        assert!(Contact::parse_value(&json!({"phone": "+79991234567"})).is_ok());
        let e = Contact::parse_value(&json!({"phone": "1"})).unwrap_err();
        assert_eq!(issues(&e), [(".phone".into(), "ru_phone".into())]);
    }

    #[cfg(feature = "openapi")]
    {
        use vld::json_schema::JsonSchema;
        let s = vld::string().min(2).rule(RuPhone).json_schema();
        assert_eq!(
            s,
            json!({"type": "string", "minLength": 2, "format": "ru-phone"})
        );
        let s = vld::string().email().rule(RuPhone).json_schema();
        assert_eq!(s["format"], "email");
        assert_eq!(s["allOf"], json!([{ "format": "ru-phone" }]));
        let r = from_fn("even", |n: &i64| n % 2 == 0).json_schema(json!({ "multipleOf": 2 }));
        let s = vld::number().int().rule(r).json_schema();
        assert_eq!(
            (s["multipleOf"].clone(), s["type"].clone()),
            (json!(2), json!("integer"))
        );
    }
}

/// Given a regex rule as a pattern source or a prebuilt `rules::Pattern`
/// When values are checked, a bad pattern is built, and the schema is exported
/// Then mismatches are `invalid_string` / `Regex`, `Pattern::new` reports the bad source as an
/// error, `string().pattern("(")` panics naming it, and a second pattern goes to `allOf`.
#[cfg(feature = "regex")]
#[test]
fn regex_patterns() {
    use vld::rules::Pattern;
    let s = vld::string().pattern(r"^[A-Z]{3}-\d{4}$");
    assert!(s.parse(r#""ABC-1234""#).is_ok());
    let e = s.parse(r#""abc""#).unwrap_err();
    assert_eq!(
        e.issues[0].code,
        IssueCode::InvalidString {
            validation: StringValidation::Regex
        }
    );
    let p = Pattern::new("^a+$").unwrap();
    assert_eq!(p.as_str(), "^a+$");
    assert_eq!(format!("{p:?}"), r#"Pattern("^a+$")"#);
    assert!(vld::string().pattern(p).parse(r#""aa""#).is_ok());

    let err = Pattern::new("(").unwrap_err();
    assert!(err.to_string().starts_with("invalid pattern"), "{err}");
    let panic = std::panic::catch_unwind(|| drop(vld::string().pattern("("))).unwrap_err();
    let msg = panic.downcast_ref::<String>().unwrap();
    assert!(
        msg.contains("vld: invalid pattern") && msg.contains("`(`"),
        "{msg}"
    );

    #[cfg(feature = "openapi")]
    {
        use vld::json_schema::JsonSchema;
        let s = vld::string().pattern("^a").pattern("b$").json_schema();
        assert_eq!(s["pattern"], "^a");
        assert_eq!(s["allOf"], json!([{ "pattern": "b$" }]));
    }
}

/// Given a custom validator that builds its own `VldError` (constructors, `push*`, the
/// `IssueBuilder`, `with_prefix`, `merge`)
/// When it reports issues and the error is sent over the wire
/// Then codes, keys, params, default and custom messages, received values and outer-first paths
/// are what was built, and the error round-trips through serde.
#[test]
fn building_errors_by_hand() {
    let code = IssueCode::TooSmall {
        origin: SizeOrigin::String,
        minimum: json!(5),
        inclusive: true,
    };
    assert_eq!(code.key(), "too_small");
    let params = code.params();
    assert!(params.contains(&("minimum", "5".to_string())), "{params:?}");
    assert!(
        params.contains(&("inclusive", "true".to_string())),
        "{params:?}"
    );
    assert_eq!(
        code.default_message(),
        "String must be at least 5 characters"
    );
    assert_eq!(
        IssueCode::Custom {
            code: "my_code".into()
        }
        .key(),
        "my_code"
    );
    assert_eq!(IssueCode::NotInt.key(), "not_int");

    // Every template parameter of a code, exactly (what `{name}` placeholders are filled with).
    let s = String::from;
    let param_table = [
        (
            IssueCode::TooSmall {
                origin: SizeOrigin::String,
                minimum: json!(1e15),
                inclusive: true,
            },
            vec![
                ("origin", s("string")),
                ("minimum", s("1000000000000000.0")),
                ("inclusive", s("true")),
            ],
        ),
        (
            IssueCode::TooBig {
                origin: SizeOrigin::Array,
                maximum: json!(999999999999999.0),
                inclusive: false,
            },
            vec![
                ("origin", s("array")),
                ("maximum", s("999999999999999")),
                ("inclusive", s("false")),
            ],
        ),
        (
            IssueCode::InvalidLength {
                origin: SizeOrigin::Tuple,
                expected: 2,
            },
            vec![("origin", s("tuple")), ("expected", s("2"))],
        ),
        (
            IssueCode::InvalidString {
                validation: StringValidation::EndsWith { suffix: s(".rs") },
            },
            vec![("validation", s("ends_with")), ("suffix", s(".rs"))],
        ),
        (
            IssueCode::InvalidString {
                validation: StringValidation::Contains { substring: s("@") },
            },
            vec![("validation", s("contains")), ("substring", s("@"))],
        ),
        (
            IssueCode::InvalidDate {
                kind: DateKind::Date,
            },
            vec![("kind", s("date"))],
        ),
        (
            IssueCode::NotInPast {
                kind: DateKind::DateTime,
            },
            vec![("kind", s("datetime"))],
        ),
        (
            IssueCode::NotInFuture {
                kind: DateKind::Time,
            },
            vec![("kind", s("time"))],
        ),
        (
            IssueCode::InvalidEnumValue {
                options: vec![s("a"), s("b")],
            },
            vec![("options", s("\"a\", \"b\""))],
        ),
        (
            IssueCode::FileTypeNotAllowed {
                kind: FileTypeKind::MediaType,
                actual: s("text/plain"),
                allowed: vec![s("image/png"), s("image/gif")],
            },
            vec![
                ("kind", s("media type")),
                ("actual", s("text/plain")),
                ("allowed", s("image/png, image/gif")),
            ],
        ),
        (
            IssueCode::FileTypeUnknown {
                kind: FileTypeKind::MagicType,
            },
            vec![("kind", s("magic type"))],
        ),
        (
            IssueCode::FileTypeUnknown {
                kind: FileTypeKind::Extension,
            },
            vec![("kind", s("extension"))],
        ),
    ];
    for (code, mut want) in param_table {
        let mut got = code.params();
        got.sort();
        want.sort();
        assert_eq!(got, want, "{code:?}");
    }
    assert_eq!(
        [
            FileTypeKind::Extension,
            FileTypeKind::MediaType,
            FileTypeKind::MagicType
        ]
        .map(|k| (k.key(), k.label())),
        [
            ("extension", "extension"),
            ("media_type", "media type"),
            ("magic_type", "magic type")
        ]
    );
    // A bound of one picks the singular template for both directions.
    for (code, want) in [
        (
            IssueCode::TooSmall {
                origin: SizeOrigin::String,
                minimum: json!(1),
                inclusive: true,
            },
            "String must be at least 1 character",
        ),
        (
            IssueCode::TooBig {
                origin: SizeOrigin::String,
                maximum: json!(1),
                inclusive: true,
            },
            "String must be at most 1 character",
        ),
    ] {
        assert_eq!(code.default_message(), want);
    }

    let issue = ValidationIssue::new(IssueCode::NotInt)
        .with_message("whole")
        .with_received(&json!(1.5));
    assert_eq!(
        (issue.message.as_str(), issue.received.clone()),
        ("whole", Some(json!(1.5)))
    );
    let e = VldError::from_code(IssueCode::NotInt);
    assert_eq!(
        (e.issues[0].message.clone(), e.issues[0].received.clone()),
        (IssueCode::NotInt.default_message(), None)
    );
    let e = VldError::from_code_with_value(IssueCode::NotInt, &json!(2.5));
    assert_eq!(e.issues[0].received, Some(json!(2.5)));
    let e = VldError::single_with_value(IssueCode::NotInt, "custom", &json!("x"));
    assert_eq!(
        (e.issues[0].message.as_str(), e.issues[0].received.clone()),
        ("custom", Some(json!("x")))
    );

    let mut e = VldError::new();
    assert!(e.is_empty());
    e.push(IssueCode::NotInt, "a");
    e.push_with_value(IssueCode::NotFinite, "b", &json!(1));
    e.push_code(IssueCode::NotUnique);
    e.push_code_with_value(IssueCode::MissingContains, &json!([]));
    assert_eq!(
        keys(&e),
        ["not_int", "not_finite", "not_unique", "missing_contains"]
    );
    assert_eq!(e.issues[1].received, Some(json!(1)));
    assert_eq!(e.issues[2].message, IssueCode::NotUnique.default_message());
    assert_eq!(e.issues[3].received, Some(json!([])));
    assert_eq!(
        VldError::single(IssueCode::NotInt, "x")
            .merge(e)
            .issues
            .len(),
        5
    );

    // with_prefix nests outer-first.
    let mut e = VldError::single(IssueCode::NotInt, "x");
    e.push(IssueCode::NotInt, "y");
    let e = e
        .with_prefix(PathSegment::Index(2))
        .with_prefix(PathSegment::Field("items".into()));
    assert_eq!(
        issues(&e),
        [
            (".items[2]".into(), "not_int".into()),
            (".items[2]".into(), "not_int".into())
        ]
    );

    let mut built = VldError::new();
    built
        .issue(IssueCode::Custom { code: "c".into() })
        .path_field("rows")
        .path_index(3)
        .path_field("é")
        .received(&json!("bad"))
        .message("m")
        .finish();
    assert_eq!(path_of(&built.issues[0]), ".rows[3].é");
    assert_eq!(
        (
            built.issues[0].message.as_str(),
            built.issues[0].received.clone()
        ),
        ("m", Some(json!("bad")))
    );
    let mut e = VldError::new();
    e.issue(IssueCode::Custom {
        code: "my_code".into(),
    })
    .finish();
    assert!(
        e.issues[0].message.contains("my_code"),
        "{}",
        e.issues[0].message
    );

    let err = vld::string().min(3).parse(r#""é""#).unwrap_err();
    let back: VldError = serde_json::from_value(serde_json::to_value(&err).unwrap()).unwrap();
    assert_eq!(
        (&back.issues[0].code, &back.issues[0].message),
        (&err.issues[0].code, &err.issues[0].message)
    );
}

/// Given a form error with nested fields (object → array → object → record)
/// When the app shows it as text (`Display`), as a prettified list, flattened per field or as a
/// tree, and logs the `received` values
/// Then paths are outer-first, `Display` lists `path: message, received …` without repeating
/// "received", root issues are form errors, long received strings are cut at 100 chars on a char
/// boundary, and container type errors carry the received value.
#[test]
fn showing_errors_on_a_form() {
    let err = vld::object()
        .field("name", vld::string().min(2))
        .field("tags", vld::array(vld::number()))
        .parse(r#"{"name": "a", "tags": [1, "x"]}"#)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        ".name: String must be at least 2 characters, received \"a\"\n\
         .tags[1]: Expected number, received string"
    );
    assert_eq!(
        vld::string()
            .parse_value(&json!(null))
            .unwrap_err()
            .to_string(),
        "Expected string, received null"
    );
    let text = vld::union(vld::string(), vld::number())
        .parse_value(&json!(true))
        .unwrap_err()
        .to_string();
    assert!(!text.contains("received boolean, received"), "{text}");

    let sample = vld::object()
        .field("name", vld::string().min(5))
        .field("age", vld::number().positive())
        .field("user", vld::object().field("email", vld::string().email()))
        .parse_value(&json!({"name": "ab", "age": -1, "user": {"email": "x"}}))
        .unwrap_err();
    let pretty = vld::format::prettify_error(&sample);
    let lines: Vec<&str> = pretty.lines().collect();
    assert_eq!(lines.len(), 6, "{pretty}");
    assert!(
        lines[0].starts_with("✖ String must be at least 5 characters"),
        "{pretty}"
    );
    assert!(lines[1].starts_with("  → at .name"), "{pretty}");
    assert!(lines[3].starts_with("  → at .age"), "{pretty}");
    assert!(lines[5].starts_with("  → at .user.email"), "{pretty}");
    let flat = vld::format::flatten_error(&sample);
    for f in ["name", "age", "user"] {
        assert_eq!(flat.field_errors[f].len(), 1, "{f}");
    }
    assert!(flat.form_errors.is_empty());
    let root = vld::format::flatten_error(&vld::string().parse("1").unwrap_err());
    assert_eq!(root.form_errors, ["Expected string, received number"]);
    assert!(root.field_errors.is_empty());
    let tree = vld::format::treeify_error(&sample);
    assert_eq!(tree.properties["name"].errors.len(), 1);
    assert_eq!(tree.properties["user"].properties["email"].errors.len(), 1);
    assert!(tree.properties["user"].errors.is_empty());
    // An issue without path and received value is one line; with a path only, the path line.
    let bare = VldError::single(IssueCode::NotInt, "whole numbers only");
    assert_eq!(vld::format::prettify_error(&bare), "✖ whole numbers only");
    assert_eq!(
        vld::format::prettify_error(&bare.with_prefix(PathSegment::Field("qty".into()))),
        "✖ whole numbers only\n  → at .qty"
    );

    // An absent required field is `missing_field` with its own wording, unless the schema
    // carries a custom type-error message.
    let form = vld::object()
        .field("name", vld::string())
        .field("nick", vld::string().type_error("nick please"));
    let err = form.parse("{}").unwrap_err();
    assert_eq!(keys(&err), ["missing_field", "missing_field"]);
    assert_eq!(
        messages(&err),
        [
            IssueCode::MissingField.default_message().as_str(),
            "nick please"
        ]
    );

    // Nested paths.
    let s = vld::object().field(
        "users",
        vld::array(
            vld::object()
                .field("name", vld::string().min(1))
                .field("tags", vld::record(vld::number().int())),
        ),
    );
    let err = s
        .parse_value(&json!({"users": [
            {"name": "ok", "tags": {}},
            {"name": "", "tags": {"a": 1, "b": "x"}}
        ]}))
        .unwrap_err();
    assert_eq!(
        issues(&err),
        [
            (".users[1].name".into(), "too_small".into()),
            (".users[1].tags.b".into(), "invalid_type".into())
        ]
    );
    let tree = vld::format::treeify_error(&err);
    let users = &tree.properties["users"].items;
    assert_eq!(users.len(), 2);
    assert!(users[0].is_none());
    let second = users[1].as_ref().unwrap();
    assert_eq!(second.properties["name"].errors.len(), 1);
    assert_eq!(second.properties["tags"].properties["b"].errors.len(), 1);

    // `received` is cut at 100 chars, never inside a char.
    let s = "é".repeat(51);
    assert_eq!(
        vld::string()
            .max(1)
            .parse_value(&json!(s))
            .unwrap_err()
            .issues[0]
            .received,
        Some(json!(s))
    );
    let exactly_100 = "é".repeat(100);
    assert_eq!(
        vld::string()
            .max(1)
            .parse_value(&json!(exactly_100))
            .unwrap_err()
            .issues[0]
            .received,
        Some(json!(exactly_100))
    );
    // `Display` shortens received strings over 50 chars to 47 + "...".
    for (len, shown) in [
        (50, format!("\"{}\"", "a".repeat(50))),
        (51, format!("\"{}...\"", "a".repeat(47))),
    ] {
        let text = vld::string()
            .max(1)
            .parse_value(&json!("a".repeat(len)))
            .unwrap_err()
            .to_string();
        assert_eq!(
            text,
            format!("String must be at most 1 character, received {shown}")
        );
    }
    // Received arrays keep 5 items, then a count of the rest.
    let arr = |n: i64| json!((0..n).collect::<Vec<_>>());
    let received = |n: i64| {
        vld::string().parse_value(&arr(n)).unwrap_err().issues[0]
            .received
            .clone()
    };
    assert_eq!(received(5), Some(arr(5)));
    assert_eq!(received(6), Some(json!([0, 1, 2, 3, 4, "... (1 more)"])));
    for long in ["é".repeat(150), "a😀".repeat(80)] {
        let err = vld::string().max(1).parse_value(&json!(long)).unwrap_err();
        let got = err.issues[0]
            .received
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(got.chars().count(), 100);
        assert!(got.ends_with("..."));
        assert!(long.starts_with(got.trim_end_matches("...")));
    }

    // Container type errors carry the received value.
    let one = Some(json!(1));
    let rec = |e: VldError| e.issues[0].received.clone();
    assert_eq!(rec(vld::array(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::object().parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::record(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::set(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(
        rec(vld::map(vld::string(), vld::string())
            .parse("1")
            .unwrap_err()),
        one
    );
    assert_eq!(rec((vld::string(),).parse("1").unwrap_err()), one);
}

/// Given decimals with scale / precision checks, `money()`, `range()` and ISO durations
/// When each fails, then its English message is read, translated to Russian, and overridden
/// with `with_messages`
/// Then scale and precision use the singular for 1 (`digit`, Russian `знака`) and the plural
/// otherwise (exact scale: `знак` / `знака` / `знаков`); `range_order` names both fields; a
/// calendar unit explains why years and months are refused; `with_messages` keys (`max_scale`,
/// `scale`, `max_precision`, `currency`, `range_order`) replace the text and survive translation.
#[test]
fn units_and_ranges_speak_plainly() {
    #[cfg(feature = "decimal")]
    {
        let s1 = vld::decimal().max_scale(1);
        assert_eq!(
            first_msg(&s1, json!("1.25")),
            "Decimal must have at most 1 digit after the point"
        );
        assert_eq!(
            first_msg(&vld::decimal().max_scale(2), json!("1.255")),
            "Decimal must have at most 2 digits after the point"
        );
        assert_eq!(
            first_msg(&vld::decimal().scale(2), json!("1.5")),
            "Decimal must have exactly 2 digits after the point"
        );
        assert_eq!(
            first_msg(&vld::decimal().max_precision(3), json!("1234")),
            "Decimal must have at most 3 digits"
        );
        #[cfg(feature = "i18n")]
        {
            let ru = |s: &ZDecimal, v: &str| {
                translate_error(&s.parse_value(&json!(v)).unwrap_err(), &translator("ru")).issues[0]
                    .message
                    .clone()
            };
            assert_eq!(
                ru(&s1, "1.25"),
                "Десятичное число должно иметь не более 1 знака после запятой"
            );
            assert_eq!(
                ru(&vld::decimal().scale(2), "1"),
                "Десятичное число должно иметь ровно 2 знака после запятой"
            );
            assert_eq!(
                ru(&vld::decimal().scale(5), "1"),
                "Десятичное число должно иметь ровно 5 знаков после запятой"
            );
        }
        let custom = vld::decimal()
            .max_scale(2)
            .max_precision(4)
            .with_messages(|k| match k {
                "max_scale" => Some("cents only".into()),
                "max_precision" => Some("too long".into()),
                "invalid_format" => Some("not a number".into()),
                _ => None,
            });
        assert_eq!(first_msg(&custom, json!("1.255")), "cents only");
        assert_eq!(first_msg(&custom, json!("12345")), "too long");
        assert_eq!(first_msg(&custom, json!("abc")), "not a number");
        let err = custom.parse_value(&json!("1.255")).unwrap_err();
        assert_eq!(messages(&translate_error(&err, &english())), ["cents only"]);
        #[cfg(feature = "i18n")]
        assert_eq!(
            messages(&translate_error(&err, &translator("ru"))),
            ["cents only"]
        );

        let wallet = vld::money().with_messages(|k| match k {
            "currency" => Some("pick a real currency".into()),
            "max_scale" => Some("too many decimals for this currency".into()),
            _ => None,
        });
        let err = wallet
            .parse_value(&json!({"amount": "1.5", "currency": "JPY"}))
            .unwrap_err();
        assert_eq!(messages(&err), ["too many decimals for this currency"]);
        let err = wallet
            .parse_value(&json!({"amount": "1", "currency": "ABC"}))
            .unwrap_err();
        assert_eq!(messages(&err), ["pick a real currency"]);
        assert_eq!(
            first_msg(&vld::money(), json!({"amount": "1.5", "currency": "JPY"})),
            "Decimal must have at most 0 digits after the point"
        );
    }

    let r = vld::range(vld::number()).keys("from", "to");
    let err = r.parse_value(&json!({"from": 2, "to": 1})).unwrap_err();
    assert_eq!(messages(&err), ["`to` must not be before `from`"]);
    #[cfg(feature = "i18n")]
    assert_eq!(
        messages(&translate_error(&err, &translator("ru"))),
        ["`to` не может быть раньше `from`"]
    );
    let r = r.with_messages(|k| (k == "range_order").then(|| "end before start".into()));
    assert_eq!(
        first_msg(&r, json!({"from": 2, "to": 1})),
        "end before start"
    );

    assert_eq!(
        first_msg(&vld::duration(), json!("P1Y")),
        "Years and months are not a fixed length: use weeks, days, hours, minutes or seconds"
    );
    #[cfg(feature = "geo")]
    {
        let ring = json!({"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1]]]});
        assert_eq!(
            first_msg(&vld::geojson(), ring),
            "Polygon ring must end with its first position"
        );
        assert_eq!(
            first_msg(&vld::geo_point(), json!("x")),
            "Expected geo point ({lat, lng} or [lng, lat]), received string"
        );
    }
}

/// The keys of one embedded catalog, as written in its JSON file.
#[cfg(feature = "i18n")]
fn catalog_keys(locale: &str) -> BTreeSet<String> {
    let json: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&shipped_catalog(locale)).unwrap();
    json.keys().cloned().collect()
}

/// Every catalog file vld ships, read from `locales/` (not only the ones this build embeds:
/// the `i18n-<lang>` features pick those).
#[cfg(feature = "i18n")]
fn shipped_locales() -> Vec<String> {
    let mut codes: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/locales"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| p.file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    codes.sort();
    codes
}

#[cfg(feature = "i18n")]
fn shipped_catalog(locale: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/locales/{locale}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Whether this build embeds `locale` (its `i18n-<lang>` feature, or the `i18n` set).
#[cfg(feature = "i18n")]
fn embedded(locale: &str) -> bool {
    vld::i18n::catalog(locale).is_some()
}

/// The embedded catalogs as an app would load them, English as the reference.
#[cfg(feature = "i18n")]
fn all_catalogs() -> vld::i18n::i18n_next::I18n {
    let files: Vec<(String, Vec<u8>)> = shipped_locales()
        .into_iter()
        .map(|l| (format!("{l}.json"), shipped_catalog(&l).into_bytes()))
        .collect();
    vld::i18n::i18n_next::I18n::builder()
        .locale("en")
        .fallback("en")
        .files(files)
        .build()
        .unwrap()
}

/// Given the JSON catalogs vld ships (the files a frontend loads too)
/// When they are checked against the key list derived from the issue codes and against English
/// Then `en.json` holds exactly the keys vld renders (every code family, sub-kind and type label,
/// nothing stale), every language has exactly English's keys, and i18n-next's own `check()` finds
/// no missing key, no placeholder that differs from English's and no plural message whose forms do
/// not fit the locale's CLDR categories (`ru` one/few/many, `ar` one/two/few/many/other, `ja` a
/// single form, …).
#[cfg(feature = "i18n")]
#[test]
fn shipped_catalogs_are_complete_and_cldr_plural() {
    let candidates: BTreeSet<String> = candidate_keys()
        .into_iter()
        .chain(["union_variant".to_string()])
        .map(|k| format!("vld.{k}"))
        .collect();
    let en = catalog_keys("en");
    let stale: Vec<_> = en.difference(&candidates).collect();
    assert!(stale.is_empty(), "keys no code renders: {stale:?}");
    for code in sample_codes() {
        let generic = code.message_keys().pop().unwrap();
        let generic = if generic == "mine" {
            "custom".into()
        } else {
            generic
        };
        assert!(en.contains(&format!("vld.{generic}")), "{generic}");
    }

    let locales = shipped_locales();
    assert_eq!(locales.len(), 29);
    for locale in &locales {
        assert_eq!(catalog_keys(locale), en, "{locale}");
    }
    // `i18n` embeds the pre-0.8 set; each `i18n-<lang>` feature adds its catalog.
    let embedded: Vec<&str> = vld::i18n::locales().collect();
    for base in ["de", "en", "es", "ru"] {
        assert!(embedded.contains(&base), "{base}");
    }
    #[cfg(feature = "i18n-all")]
    assert_eq!(embedded, locales);
    for locale in &embedded {
        assert_eq!(
            vld::i18n::catalog(locale).unwrap(),
            shipped_catalog(locale),
            "{locale}"
        );
    }
    let report = all_catalogs().check();
    assert!(report.is_clean(), "{report}");
}

/// Given an error translated with `translator(locale)` for languages with different plural rules
/// When the user reads a size message
/// Then the form follows the locale's CLDR category: Russian one / few / many (1, 3, 5, 11, 21,
/// 22), Polish and Ukrainian likewise, Arabic one / two / few / many / other (1, 2, 3, 11, 100),
/// Hebrew two, Japanese one form for every count, French singular for 0 and 1; a region or script
/// tag falls back to the language (`pt-BR` → `pt`, `zh_Hans_CN` → `zh`), an unknown locale reads
/// English, and English through the catalog equals the default message.
#[cfg(feature = "i18n")]
#[test]
fn translator_picks_cldr_plural_forms_and_falls_back() {
    let len = |locale: &str, n: usize| {
        let err = vld::string().len(n).parse_value(&json!("")).unwrap_err();
        translate_error(&err, &translator(locale)).issues[0]
            .message
            .clone()
    };
    for (n, word) in [
        (1, "символ"),
        (3, "символа"),
        (5, "символов"),
        (11, "символов"),
        (21, "символ"),
        (22, "символа"),
    ] {
        assert_eq!(
            len("ru", n),
            format!("Строка должна содержать ровно {n} {word}")
        );
    }
    // Languages outside the `i18n` set run when their feature embeds them (`i18n-all` in CI).
    if embedded("pl") {
        assert!(len("pl", 2).ends_with("2 znaki"), "{}", len("pl", 2));
        assert!(len("pl", 5).ends_with("5 znaków"), "{}", len("pl", 5));
        assert!(len("pl", 22).ends_with("22 znaki"), "{}", len("pl", 22));
    }
    if embedded("uk") {
        assert!(len("uk", 21).ends_with("21 символ"), "{}", len("uk", 21));
    }
    if embedded("ar") {
        for (n, word) in [
            (1, "حرف"),
            (2, "حرفين"),
            (3, "أحرف"),
            (11, "حرفًا"),
            (100, "حرف"),
        ] {
            assert_eq!(
                len("ar", n),
                format!("يجب أن يحتوي النص على {n} {word} بالضبط")
            );
        }
    }
    if embedded("he") {
        assert_eq!(len("he", 2), "המחרוזת חייבת להכיל בדיוק 2 תווים");
        assert_eq!(len("he", 1), "המחרוזת חייבת להכיל בדיוק 1 תו");
    }
    if embedded("ja") {
        for n in [1, 2, 5] {
            assert_eq!(
                len("ja", n),
                format!("文字列はちょうど {n} 文字である必要があります")
            );
        }
    }
    let min = |locale: &str, n: usize| {
        let err = vld::string().min(n).parse_value(&json!("")).unwrap_err();
        translate_error(&err, &translator(locale)).issues[0]
            .message
            .clone()
    };
    if embedded("fr") {
        assert!(min("fr", 1).ends_with("1 caractère"), "{}", min("fr", 1));
        assert!(min("fr", 2).ends_with("2 caractères"), "{}", min("fr", 2));
    }

    // Fallback chain: region / script dropped, unknown → English.
    assert_eq!(len("es-MX", 2), len("es", 2));
    if embedded("pt") && embedded("zh") {
        assert_eq!(len("pt-BR", 2), len("pt", 2));
        assert_eq!(len("zh_Hans_CN", 2), len("zh", 2));
    }
    assert_eq!(len("RU", 3), len("ru", 3));
    if embedded("nb") {
        assert_eq!(vld::i18n::supported_locale("no"), Some("nb"));
    }
    assert_eq!(vld::i18n::supported_locale("xx-YY"), None);
    for n in [0, 1, 2] {
        let err = vld::string().len(n).parse_value(&json!("abc")).unwrap_err();
        assert_eq!(
            translate_error(&err, &translator("xx")).issues[0].message,
            err.issues[0].message
        );
        assert_eq!(
            translate_error(&err, &translator("en")).issues[0].message,
            err.issues[0].message
        );
    }
}

/// Given an app with its own i18n-next catalog built over vld's files (`extend`) that overrides
/// one vld key in Russian, adds Esperanto with a few `vld.*` keys, and translates its own custom
/// code `vld.no_spaces`
/// When errors are translated with `Translator::new(view)`
/// Then the app's override wins (with its own plural forms), every other key keeps vld's Russian,
/// Esperanto shows the app's keys and English (the app's fallback) for the rest, the custom code
/// is translated, a custom message is still kept, and an app catalog without vld's files falls
/// back to the embedded catalog of its locale.
#[cfg(feature = "i18n")]
#[test]
fn app_catalog_overrides_keys_and_adds_languages() {
    use vld::i18n::{Translator, i18n_next::I18n};
    let app = I18n::builder()
        .locale("en")
        .fallback("en")
        .extend(vld::i18n::files())
        .messages(
            "ru",
            None,
            r#"{"vld.too_small.string": "=0 Мало | Нужен хотя бы {minimum} знак | Нужно хотя бы {minimum} знака | Нужно хотя бы {minimum} знаков",
                "title": "Регистрация"}"#,
        )
        .messages(
            "eo",
            None,
            r#"{"vld.invalid_type": "Atendis {expected}, ricevis {received}", "vld.type.string": "teksto",
                "vld.no_spaces": "Spacoj ne estas permesataj"}"#,
        )
        .build()
        .unwrap();
    let ru = Translator::new(app.with_locale("ru"));
    let tr = |e: &VldError, r: &Translator| translate_error(e, r).issues[0].message.clone();
    let short = |n| vld::string().min(n).parse_value(&json!("")).unwrap_err();
    assert_eq!(tr(&short(1), &ru), "Нужен хотя бы 1 знак");
    assert_eq!(tr(&short(3), &ru), "Нужно хотя бы 3 знака");
    assert_eq!(tr(&short(5), &ru), "Нужно хотя бы 5 знаков");
    let email = vld::string().email().parse_value(&json!("x")).unwrap_err();
    assert_eq!(tr(&email, &ru), "Некорректный адрес электронной почты");

    let eo = Translator::new(app.with_locale("eo"));
    let wrong = vld::string().parse_value(&json!(1)).unwrap_err();
    assert_eq!(tr(&wrong, &eo), "Atendis teksto, ricevis number");
    assert_eq!(tr(&email, &eo), "Invalid email address");
    let spaces = vld::string()
        .rule(from_fn("no_spaces", |s: &str| !s.contains(' ')))
        .parse_value(&json!("a b"))
        .unwrap_err();
    assert_eq!(tr(&spaces, &eo), "Spacoj ne estas permesataj");
    let custom = vld::string()
        .min_msg(3, "mine")
        .parse_value(&json!(""))
        .unwrap_err();
    assert_eq!(tr(&custom, &eo), "mine");

    // Without `extend`: the app's keys first, then vld's catalog of the app's locale.
    let bare = I18n::builder()
        .locale("de")
        .messages("de", None, r#"{"vld.missing_field": "Bitte ausfüllen"}"#)
        .build()
        .unwrap();
    let de = Translator::new(bare.with_locale("de"));
    let req = vld::object().field("a", vld::string());
    let err = req.parse_value(&json!({})).unwrap_err();
    assert_eq!(tr(&err, &de), "Bitte ausfüllen");
    assert_eq!(tr(&email, &de), tr(&email, &translator("de")));
}
