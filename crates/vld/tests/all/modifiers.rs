//! `optional`, `nullable`, `nullish`, `with_default`, `catch`, `describe`, `message`.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::prelude::*;

#[test]
fn optional_nullable_nullish_map_null_to_none() {
    let o = vld::string().optional();
    let n = vld::string().nullable();
    let nn = vld::string().nullish();
    assert_eq!(o.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(n.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(nn.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(o.parse_value(&json!("hi")).unwrap(), Some("hi".to_string()));
    assert_eq!(
        nn.parse_value(&json!("hi")).unwrap(),
        Some("hi".to_string())
    );
    // A present value is still validated.
    assert_eq!(codes(&parse_err(&o, json!(1))), ["invalid_type"]);
    assert_eq!(
        codes(&parse_err(&vld::string().min(3).nullable(), json!("a"))),
        ["too_small"]
    );
}

#[test]
fn with_default_only_replaces_null() {
    let s = vld::string().with_default("world".to_string());
    assert_eq!(s.parse_value(&json!(null)).unwrap(), "world");
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), "hello");
    assert_eq!(s.parse_value(&json!("")).unwrap(), "");
    assert_eq!(codes(&parse_err(&s, json!(42))), ["invalid_type"]);
}

#[test]
fn catch_replaces_any_failure() {
    let s = vld::string().min(6).catch("fallback".to_string());
    assert_eq!(s.parse_value(&json!("short")).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!(42)).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!(null)).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!("long enough")).unwrap(), "long enough");
}

#[test]
fn describe_keeps_validation_and_text() {
    let s = vld::string().min(3).describe("User's full name");
    assert_eq!(s.description(), "User's full name");
    assert!(s.parse(r#""Alex""#).is_ok());
    assert_eq!(codes(&s.parse(r#""Al""#).unwrap_err()), ["too_small"]);
}

#[test]
fn message_replaces_every_issue_message_and_keeps_codes() {
    let s = vld::number().int().min(0).max(100).message("Bad number");
    let err = parse_err(&s, json!("not a number"));
    assert!(err.issues.iter().all(|i| i.message == "Bad number"));
    assert_eq!(codes(&err), ["invalid_type"]);
    let err = parse_err(&vld::string().email().message("Invalid email"), json!("x"));
    assert_eq!(err.issues[0].message, "Invalid email");
    assert_eq!(codes(&err), ["invalid_string"]);
    assert_eq!(
        vld::string()
            .min(2)
            .message("e")
            .parse(r#""hello""#)
            .unwrap(),
        "hello"
    );
}

#[test]
fn default_value_accessor() {
    let s = vld::number().int().with_default(7);
    assert_eq!(*s.default_value(), 7);
    assert_eq!(s.inner_schema().parse_value(&json!(1)).unwrap(), 1);
}

/// optional / nullable / nullish differ on an absent key (and in JSON Schema).
#[test]
fn absent_key_semantics() {
    use serde_json::json;
    let obj = |s: vld::object::ZObject| s;
    let opt = obj(vld::object().field("x", vld::string().optional()));
    let nul = obj(vld::object().field("x", vld::string().nullable()));
    let nsh = obj(vld::object().field("x", vld::string().nullish()));
    for s in [&opt, &nul, &nsh] {
        assert!(s.parse_value(&json!({"x": null})).is_ok());
        assert!(s.parse_value(&json!({"x": "a"})).is_ok());
    }
    assert!(opt.parse_value(&json!({})).is_ok());
    assert!(nsh.parse_value(&json!({})).is_ok());
    let e = nul.parse_value(&json!({})).unwrap_err();
    assert_eq!(e.issues[0].code.key(), "missing_field");
    assert_eq!(e.issues[0].path_string(), ".x");

    // Wrappers keep the semantics.
    let wrapped = vld::object().field(
        "x",
        vld::string()
            .nullable()
            .describe("d")
            .refine(|_| true, "never"),
    );
    assert_eq!(
        wrapped.parse_value(&json!({})).unwrap_err().issues[0]
            .code
            .key(),
        "missing_field"
    );
}

#[cfg(feature = "derive")]
#[test]
fn absent_key_semantics_in_derive() {
    use serde_json::json;
    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    struct T {
        #[vld(vld::number().int().optional())]
        a: Option<i64>,
        #[vld(vld::number().int().nullable())]
        b: Option<i64>,
        #[vld(vld::number().int().nullish())]
        c: Option<i64>,
    }
    assert!(T::parse_value(&json!({"b": null})).is_ok());
    let e = T::parse_value(&json!({})).unwrap_err();
    assert_eq!(e.issues.len(), 1);
    assert_eq!(e.issues[0].path_string(), ".b");
}

#[cfg(feature = "openapi")]
#[test]
fn absent_key_semantics_in_json_schema() {
    use serde_json::json;
    use vld::json_schema::JsonSchema;
    let s = vld::object()
        .field("o", vld::string().optional())
        .field("n", vld::string().nullable())
        .field("h", vld::string().nullish())
        .json_schema();
    assert_eq!(s["required"], json!(["n"]));
    assert_eq!(s["properties"]["o"], json!({"type": "string"}));
    assert_eq!(
        s["properties"]["n"],
        json!({"anyOf": [{"type": "string"}, {"type": "null"}]})
    );
    assert_eq!(s["properties"]["h"], s["properties"]["n"]);
}
