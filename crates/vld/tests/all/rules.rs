//! `vld::rules`: reusable rules, `pattern()`, messages and translations.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;
use vld::rules::{Rule, from_fn};

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

fn keys(e: &vld::error::VldError) -> Vec<String> {
    e.issues.iter().map(|i| i.code.key().to_string()).collect()
}

#[test]
fn string_rule_keeps_the_builder_and_reports_its_key() {
    let s = vld::string().trim().rule(RuPhone).min(5);
    assert_eq!(s.parse(r#"" +79991234567 ""#).unwrap(), "+79991234567");
    let e = s.parse(r#""+7""#).unwrap_err();
    // Built-in and custom checks are all reported, in order.
    assert_eq!(keys(&e), ["ru_phone", "too_small"]);
    assert_eq!(
        e.issues[0].code,
        IssueCode::Custom {
            code: "ru_phone".into()
        }
    );
    assert_eq!(e.issues[0].received, Some(json!("+7")));
}

#[test]
fn message_precedence() {
    // No message anywhere: the generic `custom` template.
    let e = vld::string().rule(RuPhone).parse(r#""x""#).unwrap_err();
    assert_eq!(e.issues[0].message, "Validation error: ru_phone");
    // The rule's own message.
    let e = vld::string().rule(NoSpaces).parse(r#""a b""#).unwrap_err();
    assert_eq!(e.issues[0].message, "Spaces are not allowed");
    // with_messages by the rule key beats the rule's message.
    let e = vld::string()
        .rule(NoSpaces)
        .with_messages(|k| (k == "no_spaces").then(|| "no spaces!".to_string()))
        .parse(r#""a b""#)
        .unwrap_err();
    assert_eq!(e.issues[0].message, "no spaces!");
    // rule_msg beats everything.
    let e = vld::string()
        .rule_msg(NoSpaces, "custom text")
        .with_messages(|_| Some("bulk".to_string()))
        .parse(r#""a b""#)
        .unwrap_err();
    assert_eq!(e.issues[0].message, "custom text");
}

#[test]
fn rules_are_translated_by_key() {
    let resolver = vld::i18n::MapResolver::new(HashMap::from([(
        "ru_phone".to_string(),
        "Нужен номер +7XXXXXXXXXX".to_string(),
    )]));
    let e = vld::string().rule(RuPhone).parse(r#""8""#).unwrap_err();
    let t = vld::i18n::translate_error(&e, &resolver);
    assert_eq!(t.issues[0].message, "Нужен номер +7XXXXXXXXXX");
    // A rule's own message is a custom message: kept by translation.
    let e = vld::string().rule(NoSpaces).parse(r#""a b""#).unwrap_err();
    let t = vld::i18n::translate_error(&e, &resolver);
    assert_eq!(t.issues[0].message, "Spaces are not allowed");
}

#[test]
fn rule_on_any_schema() {
    let even = from_fn("even", |n: &i64| n % 2 == 0).message("Must be even");
    let s = vld::number().int().min(0).rule(even);
    assert_eq!(s.parse("4").unwrap(), 4);
    let e = s.parse("3").unwrap_err();
    assert_eq!(keys(&e), ["even"]);
    assert_eq!(e.issues[0].message, "Must be even");
    // The inner schema runs first: its failure is reported, the rule is not.
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
}

#[test]
fn closure_rule_on_strings_and_shared_rules() {
    let shared: Arc<dyn Rule<str>> = Arc::new(from_fn("lower", |s: &str| {
        s.chars().all(|c| !c.is_uppercase())
    }));
    let a = vld::string().rule(shared.clone());
    let b = vld::string().min(1).rule(shared);
    assert!(a.parse(r#""abc""#).is_ok());
    assert_eq!(keys(&b.parse(r#""aBc""#).unwrap_err()), ["lower"]);
}

#[cfg(feature = "derive")]
#[test]
fn rules_in_derive() {
    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    struct Contact {
        #[vld(vld::string().rule(RuPhone))]
        phone: String,
    }
    assert!(Contact::parse_value(&json!({"phone": "+79991234567"})).is_ok());
    let e = Contact::parse_value(&json!({"phone": "1"})).unwrap_err();
    assert_eq!(e.issues[0].path[0].to_string(), ".phone");
    assert_eq!(keys(&e), ["ru_phone"]);
}

#[cfg(feature = "openapi")]
mod schema {
    use super::*;
    use vld::json_schema::JsonSchema;

    #[test]
    fn rule_keywords_merge_into_the_schema() {
        let s = vld::string().min(2).rule(RuPhone).json_schema();
        assert_eq!(
            s,
            json!({"type": "string", "minLength": 2, "format": "ru-phone"})
        );
        // A keyword the schema already has goes to allOf, nothing is lost.
        let s = vld::string().email().rule(RuPhone).json_schema();
        assert_eq!(s["format"], "email");
        assert_eq!(s["allOf"], json!([{ "format": "ru-phone" }]));
    }

    #[test]
    fn generic_rule_schema() {
        let r = from_fn("even", |n: &i64| n % 2 == 0).json_schema(json!({ "multipleOf": 2 }));
        let s = vld::number().int().rule(r).json_schema();
        assert_eq!(s["multipleOf"], 2);
        assert_eq!(s["type"], "integer");
    }
}

#[cfg(feature = "regex")]
mod pattern {
    use super::*;
    use vld::error::StringValidation;
    use vld::rules::Pattern;

    #[test]
    fn pattern_from_source_or_prebuilt() {
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
        assert!(vld::string().pattern(p).parse(r#""aa""#).is_ok());
    }

    #[test]
    fn invalid_pattern() {
        let err = Pattern::new("(").unwrap_err();
        assert!(err.to_string().starts_with("invalid pattern"), "{err}");
        let panic = std::panic::catch_unwind(|| drop(vld::string().pattern("("))).unwrap_err();
        let msg = panic.downcast_ref::<String>().unwrap();
        assert!(
            msg.contains("vld: invalid pattern") && msg.contains("`(`"),
            "{msg}"
        );
    }

    #[cfg(feature = "openapi")]
    #[test]
    fn pattern_schema() {
        use vld::json_schema::JsonSchema;
        let s = vld::string().pattern("^a").pattern("b$").json_schema();
        assert_eq!(s["pattern"], "^a");
        assert_eq!(s["allOf"], json!([{ "pattern": "b$" }]));
    }
}
