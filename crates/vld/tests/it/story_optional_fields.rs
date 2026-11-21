//! Stories: optional, nullable and defaulted fields of a profile update — what `null`, an absent
//! key and a bad value do under `optional` / `nullable` / `nullish` / `with_default` / `catch`,
//! through wrappers (`describe`, `refine`), `schema!`, derive and the JSON Schema.
//!
//! Random trees of these modifiers inside objects are the model in `model_schema.rs`
//! (`optional`, `nullable`, `nullish`, `with_default`, `catch`, absent keys → `missing_field`).

use crate::common::*;

use serde_json::json;
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    struct Person {
        name: String => vld::string(),
        nick: Option<String> => vld::string().optional(),
    }
}

fn ok(code: &str, path: &str) -> Vec<(String, String)> {
    vec![(path.to_string(), code.to_string())]
}

/// Given a profile-update form with an optional nickname, a nullable bio, a nullish avatar, a
/// greeting with a default, a theme that falls back on anything bad, and a described name
/// When the client sends values, `null`, or leaves keys out
/// Then `null` maps to `None` / the default / the fallback, a present value is still validated,
/// `with_default` replaces only `null` (not `""`), `catch` replaces every failure, an absent
/// `nullable` key (also behind `describe` / `refine`) and an absent required key are
/// `missing_field` while a present `null` is a type error, and the wrappers keep their inner
/// schema and text.
#[test]
fn null_and_absent_keys_in_a_profile_update() {
    let o = vld::string().optional();
    let n = vld::string().nullable();
    let nn = vld::string().nullish();
    for v in [json!(null), json!("hi")] {
        let want = v.as_str().map(String::from);
        assert_eq!(o.parse_value(&v).unwrap(), want);
        assert_eq!(n.parse_value(&v).unwrap(), want);
        assert_eq!(nn.parse_value(&v).unwrap(), want);
    }
    assert_eq!(
        issues(&o.parse_value(&json!(1)).unwrap_err()),
        ok("invalid_type", "")
    );
    assert_eq!(
        issues(
            &vld::string()
                .min(3)
                .nullable()
                .parse_value(&json!("a"))
                .unwrap_err()
        ),
        ok("too_small", "")
    );

    let greeting = vld::string().with_default("world".to_string());
    assert_eq!(greeting.parse_value(&json!(null)).unwrap(), "world");
    assert_eq!(greeting.parse_value(&json!("hello")).unwrap(), "hello");
    assert_eq!(greeting.parse_value(&json!("")).unwrap(), "");
    assert_eq!(
        issues(&greeting.parse_value(&json!(42)).unwrap_err()),
        ok("invalid_type", "")
    );
    let count = vld::number().int().with_default(7);
    assert_eq!(*count.default_value(), 7);
    assert_eq!(count.inner_schema().parse_value(&json!(1)).unwrap(), 1);

    let theme = vld::string().min(6).catch("fallback".to_string());
    for bad in [json!("short"), json!(42), json!(null)] {
        assert_eq!(theme.parse_value(&bad).unwrap(), "fallback");
    }
    assert_eq!(
        theme.parse_value(&json!("long enough")).unwrap(),
        "long enough"
    );

    let name = vld::string().min(3).describe("User's full name");
    assert_eq!(name.description(), "User's full name");
    assert!(name.parse(r#""Alex""#).is_ok());
    assert_eq!(
        issues(&name.parse(r#""Al""#).unwrap_err()),
        ok("too_small", "")
    );

    // Absent keys.
    let opt = vld::object().field("x", vld::string().optional());
    let nul = vld::object().field("x", vld::string().nullable());
    let nsh = vld::object().field("x", vld::string().nullish());
    for s in [&opt, &nul, &nsh] {
        assert!(s.parse_value(&json!({"x": null})).is_ok());
        assert!(s.parse_value(&json!({"x": "a"})).is_ok());
    }
    assert!(opt.parse_value(&json!({})).is_ok());
    assert!(nsh.parse_value(&json!({})).is_ok());
    assert_eq!(
        issues(&nul.parse_value(&json!({})).unwrap_err()),
        ok("missing_field", ".x")
    );
    let wrapped = vld::object().field(
        "x",
        vld::string()
            .nullable()
            .describe("d")
            .refine(|_| true, "never"),
    );
    assert_eq!(
        issues(&wrapped.parse_value(&json!({})).unwrap_err()),
        ok("missing_field", ".x")
    );

    // A required key: absent → missing_field alone, present null → type error.
    assert_eq!(
        issues(&Person::parse_value(&json!({})).unwrap_err()),
        ok("missing_field", ".name")
    );
    assert_eq!(
        issues(&Person::parse_value(&json!({"name": null})).unwrap_err()),
        ok("invalid_type", ".name")
    );
    let obj = vld::object().field("name", vld::string());
    assert_eq!(
        issues(&obj.parse_value(&json!({})).unwrap_err()),
        ok("missing_field", ".name")
    );
    // partial().required() makes the field required again.
    let req = vld::object().field("a", vld::string()).partial().required();
    assert_eq!(
        issues(&req.parse_value(&json!({"a": null})).unwrap_err()),
        ok("missing_field", ".a")
    );
}

/// Given the same three modifiers on derived struct fields and in an exported JSON Schema
/// When an empty document is parsed and the schema is read by a client
/// Then only the `nullable` field is reported (as `.b`), and the schema marks only it required:
/// `optional` is the bare type, `nullable` and `nullish` are `anyOf [type, null]`; a `time`
/// datetime default is RFC 3339 behind every wrapper and inside unions.
#[cfg(any(feature = "derive", feature = "openapi"))]
#[test]
fn absent_key_rules_hold_in_derive_and_json_schema() {
    #[cfg(feature = "derive")]
    {
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
        assert_eq!(
            issues(&T::parse_value(&json!({})).unwrap_err()),
            ok("missing_field", ".b")
        );
    }

    #[cfg(feature = "openapi")]
    {
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

        // A `time` datetime serializes as non-RFC 3339 text; its default stays RFC 3339
        // behind every wrapper and inside unions.
        #[cfg(feature = "time")]
        {
            use vld::combinators::{Either, Either3};
            use vld::time::datetime;
            let at = datetime().parse(r#""2024-01-02T03:04:05Z""#).unwrap();
            let want = json!("2024-01-02T03:04:05Z");
            let pre = vld::preprocess(|v: &serde_json::Value| v.clone(), datetime());
            let defaults = [
                datetime().catch(at).with_default(at).json_schema(),
                datetime().message("m").with_default(at).json_schema(),
                pre.with_default(at).json_schema(),
                datetime()
                    .refine(|_| true, "m")
                    .with_default(at)
                    .json_schema(),
                datetime()
                    .super_refine(|_, _| {})
                    .with_default(at)
                    .json_schema(),
                datetime().with_default(at).with_default(at).json_schema(),
                vld::union(datetime(), vld::string())
                    .with_default(Either::Left(at))
                    .json_schema(),
                vld::union3(datetime(), vld::string(), vld::boolean())
                    .with_default(Either3::First(at))
                    .json_schema(),
            ];
            for (i, js) in defaults.iter().enumerate() {
                assert_eq!(js["default"], want, "wrapper #{i}");
            }
        }
    }
}
