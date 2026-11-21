//! Stories: schemars types validated by vld — `impl_vld_parse!` gives
//! `VldParse` (vld extractors) and `SchemarsValidate` (validate an instance,
//! validate JSON, validate + deserialize) from the type's schemars schema.

use super::bridge::schemars;
use super::bridge::*;
use crate::common::{invalid, pairs, valid};
use serde_json::json;
use vld::schema::VldParse;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Item {
    name: String,
    qty: u32,
}
impl_vld_parse!(Item);

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Inner {
    #[schemars(length(min = 3))]
    code: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Outer {
    inner: Inner,
    pair: (u8, String),
}
impl_vld_parse!(Outer);

/// Schema says "string", serde wants a number: validation passes,
/// deserialization does not.
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Mismatch {
    #[schemars(with = "String")]
    n: u32,
}
impl_vld_parse!(Mismatch);

/// Given a schemars type with `impl_vld_parse!`, when a client sends good and
/// bad bodies, then vld's `VldParse` and every `SchemarsValidate` entry point
/// accept the good one (typed) and report the bad ones as `(path, code)`.
#[test]
fn a_schemars_type_parses_through_vld() {
    // VldParse: what vld extractors call.
    let item = Item::vld_parse_value(&json!({"name": "Widget", "qty": 5})).unwrap();
    assert_eq!((item.name.as_str(), item.qty), ("Widget", 5));
    let err = Item::vld_parse_value(&json!({"name": "Widget"})).unwrap_err();
    assert_eq!(
        pairs(&err),
        [(".qty".to_string(), "missing_field".to_string())]
    );
    let err = Item::vld_parse_value(&json!({"name": 123, "qty": "x"})).unwrap_err();
    let got = pairs(&err);
    assert!(
        got.contains(&(".name".into(), "invalid_type".into())),
        "{got:?}"
    );
    assert!(
        got.contains(&(".qty".into(), "invalid_type".into())),
        "{got:?}"
    );

    // SchemarsValidate on an instance and on JSON.
    Item {
        name: "Widget".into(),
        qty: 5,
    }
    .vld_validate()
    .unwrap();
    Item::vld_validate_json(&json!({"name": "Alice", "qty": 30})).unwrap();
    let err = Item::vld_validate_json(&json!({"name": "Alice"})).unwrap_err();
    assert_eq!(
        pairs(&err),
        [(".qty".to_string(), "missing_field".to_string())]
    );
    let err = Item::vld_validate_json(&json!({"name": 1, "qty": -1})).unwrap_err();
    let got = pairs(&err);
    assert!(
        got.contains(&(".name".into(), "invalid_type".into())),
        "{got:?}"
    );
    assert!(
        got.contains(&(".qty".into(), "too_small".into())),
        "{got:?}"
    );

    // vld_parse: validate, then deserialize.
    let item = Item::vld_parse(&json!({"name": "Gadget", "qty": 10})).unwrap();
    assert_eq!((item.name.as_str(), item.qty), ("Gadget", 10));
    match Item::vld_parse(&json!({"name": "Bob"})) {
        Err(e @ VldSchemarsError::Validation(_)) => {
            let VldSchemarsError::Validation(inner) = &e else {
                unreachable!()
            };
            assert_eq!(
                pairs(inner),
                [(".qty".to_string(), "missing_field".to_string())]
            );
            assert!(std::error::Error::source(&e).is_some());
            assert!(
                e.to_string().starts_with("Schema validation error: "),
                "{e}"
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Given a field whose schema and serde shape disagree, when a body passes the
/// schema, then `vld_parse` reports a deserialization error and `VldParse` a
/// `parse_error`; an instance that cannot serialize is a `parse_error` too.
#[test]
fn deserialization_failures_are_reported_after_validation() {
    let body = json!({"n": "seven"});
    Mismatch::vld_validate_json(&body).unwrap();
    match Mismatch::vld_parse(&body) {
        Err(e @ VldSchemarsError::Deserialization(_)) => {
            assert!(std::error::Error::source(&e).is_none());
            assert!(e.to_string().starts_with("Deserialization error: "), "{e}");
        }
        other => panic!("expected a deserialization error, got {other:?}"),
    }
    let err = Mismatch::vld_parse_value(&body).unwrap_err();
    assert_eq!(pairs(&err), [(String::new(), "parse_error".to_string())]);
    // The instance serializes as a number, which its own schema rejects.
    let err = Mismatch { n: 7 }.vld_validate().unwrap_err();
    assert_eq!(
        pairs(&err),
        [(".n".to_string(), "invalid_type".to_string())]
    );

    // From<VldError>: a validation error converts.
    let e: VldSchemarsError =
        vld::error::VldError::single(vld::error::IssueCode::ParseError, "x").into();
    assert!(matches!(e, VldSchemarsError::Validation(_)));
}

/// Given nested schemars structs with a tuple and `deny_unknown_fields`, when
/// bodies are validated, then the validator follows the `$ref` into the inner
/// struct, checks tuple positions and rejects unknown keys.
#[test]
fn nested_schemars_structs_validate_deeply() {
    let schema = generate_from_schemars::<Outer>();
    valid(
        &schema,
        &json!({"inner": {"code": "abc"}, "pair": [1, "x"]}),
    );
    invalid(
        &schema,
        &json!({"inner": {"code": "a"}, "pair": [1, "x"]}),
        &[(".inner.code", "too_small")],
    );
    invalid(
        &schema,
        &json!({"inner": {"code": "abc"}, "pair": ["no", "x"]}),
        &[(".pair[0]", "invalid_type")],
    );
    invalid(
        &schema,
        &json!({"inner": {"code": "abc"}, "pair": [1, "x"], "extra": 1}),
        &[(".extra", "unrecognized_field")],
    );
    let outer =
        Outer::vld_parse_value(&json!({"inner": {"code": "abc"}, "pair": [7, "x"]})).unwrap();
    assert_eq!((outer.inner.code.as_str(), outer.pair.0), ("abc", 7));
    Outer {
        inner: Inner {
            code: "abcd".into(),
        },
        pair: (1, "y".into()),
    }
    .vld_validate()
    .unwrap();

    // Plain types work too.
    for (schema, ok, bad) in [
        (generate_from_schemars::<String>(), json!("s"), json!(1)),
        (generate_from_schemars::<i32>(), json!(-3), json!("x")),
        (generate_from_schemars::<bool>(), json!(true), json!(0)),
    ] {
        assert!(schema.is_object(), "{schema}");
        valid(&schema, &ok);
        invalid(&schema, &bad, &[("", "invalid_type")]);
    }
    assert_eq!(
        schemars_to_json(&generate_schemars::<i32>()),
        generate_from_schemars::<i32>()
    );
}
