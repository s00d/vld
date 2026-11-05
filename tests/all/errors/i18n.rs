//! `vld::i18n`: resolvers, built-in sets, key fallback, exclusive bounds.

use std::collections::HashMap;
use vld::i18n::{english, translate_error, translate_issue, FnResolver, MapResolver};
use vld::prelude::*;

#[test]
fn map_and_fn_resolvers_fill_params() {
    let mut map = HashMap::new();
    map.insert("too_small".into(), "Минимум {minimum}".into());
    let err = vld::string().min(5).parse(r#""ab""#).unwrap_err();
    let tr = translate_error(&err, &MapResolver::new(map));
    assert_eq!(tr.issues[0].message, "Минимум 5");

    let resolver = FnResolver::new(|key| match key {
        "too_small" => Some("Zu kurz! Min: {minimum}".into()),
        _ => None,
    });
    let tr = translate_error(&err, &resolver);
    assert_eq!(tr.issues[0].message, "Zu kurz! Min: 5");
    assert_eq!(tr.issues[0].code, err.issues[0].code);
    assert_eq!(tr.issues[0].path, err.issues[0].path);
}

/// The most specific key wins (`too_small.string` over `too_small`).
#[test]
fn specific_key_before_generic() {
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
}

#[test]
fn untranslated_keys_keep_original_message() {
    let err = vld::string().email().parse(r#""bad""#).unwrap_err();
    let tr = translate_error(&err, &FnResolver::new(|_| None));
    assert_eq!(tr.issues[0].message, err.issues[0].message);
}

#[test]
fn english_matches_default_messages() {
    let errs = [
        vld::string().min(3).parse(r#""a""#).unwrap_err(),
        vld::number().max(3.0).parse("4").unwrap_err(),
        vld::number().int().parse("1.5").unwrap_err(),
        vld::array(vld::any()).unique().parse("[1, 1]").unwrap_err(),
    ];
    for err in errs {
        let issue = &err.issues[0];
        assert_eq!(translate_issue(issue, &english()).message, issue.message);
    }
}

#[test]
fn exclusive_bounds_say_greater_than() {
    let err = vld::number().positive().parse("0").unwrap_err();
    assert_eq!(
        translate_error(&err, &english()).issues[0].message,
        "Number must be greater than 0"
    );
    let err = vld::number().min(3.0).parse("1").unwrap_err();
    assert_eq!(
        translate_error(&err, &english()).issues[0].message,
        "Number must be at least 3"
    );
    // A resolver with only the generic keys still distinguishes exclusive bounds.
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
}

#[test]
fn builtin_sets_translate_and_keep_params() {
    let err = vld::number().min(10.0).parse("5").unwrap_err();
    let en = translate_error(&err, &english()).issues[0].message.clone();
    for resolver in [
        vld::i18n::russian(),
        vld::i18n::german(),
        vld::i18n::spanish(),
    ] {
        let msg = translate_error(&err, &resolver).issues[0].message.clone();
        assert!(msg.contains("10"), "{msg}");
        assert_ne!(msg, en, "translated set must differ from English");
    }
}
