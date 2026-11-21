//! Stories: API request types with `#[derive(Validate)]`: parse a body, read
//! the issues (paths are the JSON keys serde uses), validate instances under
//! the serialize names, parse leniently, emit JSON Schema / OpenAPI with
//! nested components. Enum derives are in `story_tagged_enums.rs`.

use crate::common::*;
use crate::story_tagged_enums::{Point, agree, same_as_serde};

use serde::{Deserialize, Serialize};
use serde_json::json;
use vld::Validate;
use vld::schema::VldParse;

fn iss(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

#[derive(Debug, Validate)]
struct SimpleUser {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().min(0).optional())]
    age: Option<i64>,
}

/// No `Default` / `Serialize`: must still derive (lenient is opt-in).
#[derive(Debug)]
pub struct Opaque(pub String);

#[derive(Debug, Validate)]
pub struct HasOpaque {
    #[vld(vld::string().transform(Opaque))]
    pub inner: Opaque,
}

#[derive(Debug, Validate)]
pub struct RawDerive {
    #[vld(vld::string())]
    pub r#type: String,
}

#[derive(Debug, Validate)]
struct Order {
    #[vld(vld::string().try_transform(|s| s.parse::<u128>()))]
    amount: u128,
}

/// Given a `CreateUser` body type deriving `Validate` (string bounds, email,
/// optional age), a field transformed into an opaque type, a raw-identifier
/// field and a `try_transform` into `u128`
/// When bodies are parsed through `parse_value`, `vld_parse` and
/// `vld_parse_value`
/// Then valid bodies give typed values (absent optional = `None`); a bad body
/// reports every field in declaration order with its code; a non-object is
/// `invalid_type` expecting `object`; `r#type` is the JSON key `type`; a
/// failing `try_transform` is a `transform` issue carrying the parse error's
/// text, and the inner schema's own error stops it first.
#[test]
fn create_user_body_is_parsed_and_every_bad_field_reported() {
    let u =
        SimpleUser::parse_value(&json!({"name": "Alice", "email": "a@x.com", "age": 30})).unwrap();
    assert_eq!(
        (u.name.as_str(), u.email.as_str(), u.age),
        ("Alice", "a@x.com", Some(30))
    );
    let u = SimpleUser::vld_parse(r#"{"name": "Bob", "email": "b@x.com"}"#).unwrap();
    assert_eq!(u.age, None);
    let u = SimpleUser::vld_parse_value(&json!({"name": "Bob", "email": "b@x.com"})).unwrap();
    assert_eq!(u.name, "Bob");

    let err =
        SimpleUser::parse_value(&json!({"name": "X", "email": "nope", "age": -5})).unwrap_err();
    assert_eq!(
        issues(&err),
        iss(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_small")
        ])
    );
    for bad in [json!("str"), json!([1, 2])] {
        let err = SimpleUser::parse_value(&bad).unwrap_err();
        assert_eq!(
            err.issues[0].code,
            vld::error::IssueCode::InvalidType {
                expected: "object".into(),
                received: if bad.is_string() { "string" } else { "array" }.into(),
            }
        );
    }

    assert_eq!(
        HasOpaque::parse_value(&json!({"inner": "x"}))
            .unwrap()
            .inner
            .0,
        "x"
    );
    assert_eq!(
        RawDerive::parse_value(&json!({"type": "a"}))
            .unwrap()
            .r#type,
        "a"
    );
    assert_eq!(
        issues(&RawDerive::parse_value(&json!({})).unwrap_err()),
        iss(&[(".type", "missing_field")])
    );

    let big = "340282366920938463463374607431768211455";
    assert_eq!(
        Order::parse_value(&json!({ "amount": big }))
            .unwrap()
            .amount,
        u128::MAX
    );
    let e = Order::parse_value(&json!({"amount": "12x"})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".amount", "transform")]));
    assert_eq!(e.issues[0].message, "invalid digit found in string");
    let e = Order::parse_value(&json!({"amount": 5})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".amount", "invalid_type")]));
}

#[derive(Debug, Serialize, Validate)]
struct CamelUser {
    #[serde(rename = "firstName")]
    #[vld(vld::string().min(2))]
    first_name: String,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "UPPERCASE")]
pub struct Serdeish {
    #[vld(vld::string())]
    pub first_name: String,
    #[serde(rename(serialize = "ser", deserialize = "de"))]
    #[vld(vld::string())]
    pub renamed: String,
    #[serde(skip)]
    pub skipped: Vec<u8>,
    #[serde(default)]
    #[vld(vld::number().int().min(10))]
    pub defaulted: i64,
    #[serde(default = "seven")]
    #[vld(vld::number().int())]
    pub with_fn: i64,
}

fn seven() -> i64 {
    7
}

#[derive(Debug, Serialize, Validate)]
#[serde(rename_all(serialize = "camelCase", deserialize = "kebab-case"))]
struct Split {
    #[serde(rename(serialize = "a", deserialize = "b"))]
    #[vld(vld::string().min(1))]
    field: String,
    #[vld(vld::string())]
    other_name: String,
}

/// Given request types using serde's `rename`, `rename_all` (also split into
/// serialize / deserialize), `skip`, `default` and `default = "fn"`
/// When bodies are parsed, instances validated and the JSON Schema emitted
/// Then parsing reads the deserialize names only and reports issues under
/// them; skipped fields are ignored and defaulted; absent `default` fields
/// take the type default or the function's value, present ones are still
/// validated; `validate` / `is_valid` read the serialize names (what serde
/// writes); defaulted and skipped fields are not `required` in the schema.
#[test]
fn serde_attributes_decide_the_json_keys() {
    let u = CamelUser::parse_value(&json!({"firstName": "John"})).unwrap();
    assert_eq!(u.first_name, "John");
    assert_eq!(
        issues(&CamelUser::parse_value(&json!({"first_name": "John"})).unwrap_err()),
        iss(&[(".firstName", "missing_field")])
    );

    let v = Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "skipped": "ignored"}))
        .unwrap();
    assert_eq!((v.first_name.as_str(), v.renamed.as_str()), ("a", "b"));
    assert!(v.skipped.is_empty());
    assert_eq!((v.defaulted, v.with_fn), (0, 7));
    let err =
        Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "DEFAULTED": 1})).unwrap_err();
    assert_eq!(issues(&err), iss(&[(".DEFAULTED", "too_small")]));

    let v = Split::parse_value(&json!({"b": "x", "other-name": "y"})).unwrap();
    assert!(Split::validate(&v).is_ok());
    assert!(Split::is_valid(&json!({"a": "x", "otherName": "y"})));
    assert!(!Split::is_valid(&json!({"b": "x", "other-name": "y"})));

    #[cfg(feature = "openapi")]
    {
        let js = Serdeish::json_schema();
        assert_eq!(js["required"], json!(["FIRST_NAME", "de"]));
        assert!(js["properties"].get("skipped").is_none());
    }
}

#[derive(Debug, Default, Validate)]
#[vld(lenient)]
struct Config {
    #[vld(vld::string().min(1))]
    host: String,
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
}

/// Given a `#[vld(lenient)]` config type
/// When a partly invalid config is checked field by field and parsed leniently
/// Then `validate_fields_value` gives one result per field, and
/// `parse_lenient_value` keeps the valid fields, defaults the invalid ones and
/// counts the errors.
#[test]
fn lenient_config_keeps_the_valid_fields() {
    let fields = Config::validate_fields_value(&json!({"host": "", "port": 0})).unwrap();
    assert_eq!(
        fields.iter().map(|f| f.is_err()).collect::<Vec<_>>(),
        [true, true]
    );
    let r = Config::parse_lenient_value(&json!({"host": "h", "port": 0})).unwrap();
    assert_eq!(r.error_count(), 1);
    assert_eq!((r.value.host.as_str(), r.value.port), ("h", 0));
}

#[derive(Debug, Validate)]
pub struct Page<T: VldParse> {
    #[vld(vld::array(vld::nested(T::vld_parse_value)))]
    pub items: Vec<T>,
}

#[derive(Debug, Validate)]
pub struct Item {
    #[vld(vld::number().int())]
    pub id: i64,
}

#[derive(Debug, Validate)]
#[allow(dead_code)] // fields exist for their schemas
pub struct DerivedAddr {
    #[vld(vld::string())]
    pub city: String,
}

#[derive(Debug, Validate)]
#[allow(dead_code)]
pub struct DerivedUser {
    #[vld(vld::nested!(DerivedAddr))]
    pub addr: DerivedAddr,
    #[vld(vld::nested!(Page<Item>).optional())]
    pub page: Option<Page<Item>>,
}

/// Given a generic `Page<T>` of nested items and a user type nesting derived
/// types through `vld::nested!`
/// When pages are parsed, raw values validated and an OpenAPI document emitted
/// Then nested issues carry the full path (`.items[1].id`); `is_valid` /
/// `validate` accept any serializable value; the nested types are collected
/// (generic names sanitized: `Page_Item_`), become components and are
/// referenced by `$ref`.
#[test]
fn generic_pages_and_nested_components() {
    let p = Page::<Item>::parse_value(&json!({"items": [{"id": 1}]})).unwrap();
    assert_eq!(p.items[0].id, 1);
    assert_eq!(
        issues(
            &Page::<Item>::parse_value(&json!({"items": [{"id": 1}, {"id": "x"}]})).unwrap_err()
        ),
        iss(&[(".items[1].id", "invalid_type")])
    );

    #[derive(Serialize)]
    struct Raw {
        id: i64,
    }
    assert!(Item::is_valid(&Raw { id: 1 }));
    assert_eq!(
        issues(&Item::validate(&json!({"id": "x"})).unwrap_err()),
        iss(&[(".id", "invalid_type")])
    );

    let u = DerivedUser::parse_value(&json!({"addr": {"city": "x"}})).unwrap();
    assert!(u.page.is_none());

    #[cfg(feature = "openapi")]
    {
        let names: Vec<&str> = DerivedUser::__vld_nested_schemas()
            .iter()
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(names, ["DerivedAddr", "Page_Item_"]);
        let doc = DerivedUser::to_openapi_document();
        let schemas = &doc["components"]["schemas"];
        assert!(schemas["DerivedAddr"].is_object());
        assert!(schemas["Page_Item_"].is_object());
        assert_eq!(
            schemas["DerivedUser"]["properties"]["page"]["$ref"],
            "#/components/schemas/Page_Item_"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
struct Meta {
    #[vld(vld::string().min(1))]
    author: String,
    #[vld(vld::number().int().min(1).optional())]
    version: Option<i64>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Validate)]
struct Doc {
    #[vld(vld::string().min(1))]
    title: String,
    #[serde(flatten)]
    meta: Meta,
    #[serde(flatten)]
    #[vld(vld::nested(Point::parse_value))]
    at: Point,
}

/// Given a document type flattening a derived `Meta` and an opaque
/// `vld::nested(Point::parse_value)`
/// When documents are parsed, an instance validated and the schema emitted
/// Then valid input equals serde's value; issues of flattened fields keep
/// their own top-level paths; the derived type's properties are inlined
/// (`required` = `title`, `author`) while the opaque one goes into `allOf`; a
/// validator accepts a full document and rejects one without `author`.
#[test]
fn flattened_document_fields() {
    same_as_serde::<Doc>(json!({"title": "t", "author": "a", "x": 1, "y": 2}));
    same_as_serde::<Doc>(json!({"title": "t", "author": "a", "version": 3, "x": 1, "y": 2}));
    assert!(Doc::is_valid(&Doc {
        title: "t".into(),
        meta: Meta {
            author: "a".into(),
            version: None,
        },
        at: Point { x: 0, y: 0 },
    }));
    let e = Doc::parse_value(&json!({"title": "", "author": "", "x": -1, "y": 0})).unwrap_err();
    assert_eq!(
        issues(&e),
        iss(&[
            (".title", "too_small"),
            (".author", "too_small"),
            (".x", "too_small")
        ])
    );

    #[cfg(feature = "openapi")]
    {
        let s = Doc::json_schema();
        let props: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
        for k in ["title", "author", "version"] {
            assert!(props.iter().any(|p| *p == k), "{k} missing in {s:#}");
        }
        assert_eq!(s["allOf"], json!([{"type": "object"}]));
        assert_eq!(s["required"], json!(["title", "author"]));
        let v = jsonschema::draft202012::options().build(&s).unwrap();
        assert!(v.is_valid(&json!({"title": "t", "author": "a", "x": 1, "y": 2})));
        assert!(!v.is_valid(&json!({"title": "t", "x": 1, "y": 2})));
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Strict {
    #[vld(vld::string())]
    user_name: String,
    #[serde(default)]
    #[vld(vld::number().int())]
    age: i64,
}

/// Given a request type with `#[serde(deny_unknown_fields)]`
/// When bodies with extra keys (or the Rust field name) are parsed
/// Then vld rejects exactly what serde rejects, each extra key an
/// `unrecognized_field` at its path; `validate` accepts an instance (serialize
/// names); the schema has `additionalProperties: false`.
#[test]
fn deny_unknown_fields_on_a_request() {
    agree::<Strict>(json!({"userName": "a"}), &[]);
    agree::<Strict>(json!({"userName": "a", "age": 3}), &[]);
    agree::<Strict>(json!({"userName": "a", "extra": 1}), &[".extra"]);
    agree::<Strict>(json!({"userName": "a", "user_name": "b"}), &[".user_name"]);
    assert!(Strict::is_valid(&Strict {
        user_name: "a".into(),
        age: 1
    }));

    #[cfg(feature = "openapi")]
    assert_eq!(Strict::json_schema()["additionalProperties"], false);
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Wizard {
    #[serde(rename = "userName")]
    #[vld(vld::string().min(3))]
    pub user_name: String,
    #[serde(default)]
    #[vld(vld::number().int().min(18))]
    pub age: i64,
    #[vld(vld::string().email().optional())]
    pub email: Option<String>,
    #[serde(skip_serializing)]
    #[vld(vld::string().min(8))]
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[vld(vld::string())]
    pub note: String,
    pub rgb: [u8; 3],
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Default, Serialize, Deserialize, Validate)]
pub struct Extra {
    #[serde(default)]
    #[vld(vld::string())]
    pub theme: String,
}

/// Given a sign-up wizard type with a renamed, a defaulted, an optional, a
/// `skip_serializing` and a `skip_serializing_if` field, a `[u8; 3]` field
/// without `#[vld]` and a flattened part
/// When each wizard step checks one field with `validate_field`, the whole body
/// is parsed, the parsed instance is validated and the JSON Schema emitted
/// Then a field is checked by its deserialize name alone (`None` = absent key:
/// `missing_field` when required, Ok when defaulted / optional), issues carry
/// the field path, unknown and flattened names are `unrecognized_field`; the
/// array field takes exactly 3 items (`invalid_length` otherwise, items
/// checked as `u8`); `validate()` accepts the serialized instance although
/// serde left the skipped fields out, while parsing still requires them; the
/// array schema has `minItems = maxItems = 3`.
#[test]
fn wizard_checks_one_field_at_a_time() {
    let one = |name: &str, v: Option<serde_json::Value>| {
        Wizard::validate_field(name, v.as_ref()).map_err(|e| issues(&e))
    };
    assert_eq!(one("userName", Some(json!("bob"))), Ok(()));
    assert_eq!(
        one("userName", Some(json!("bo"))),
        Err(iss(&[(".userName", "too_small")]))
    );
    assert_eq!(
        one("userName", None),
        Err(iss(&[(".userName", "missing_field")]))
    );
    assert_eq!(one("age", None), Ok(()));
    assert_eq!(
        one("age", Some(json!(12))),
        Err(iss(&[(".age", "too_small")]))
    );
    assert_eq!(one("email", None), Ok(()));
    assert_eq!(
        one("email", Some(json!("x"))),
        Err(iss(&[(".email", "invalid_string")]))
    );
    assert_eq!(one("rgb", Some(json!([1, 2, 3]))), Ok(()));
    assert_eq!(
        one("rgb", Some(json!([1, 2]))),
        Err(iss(&[(".rgb", "invalid_length")]))
    );
    assert_eq!(
        one("rgb", Some(json!([1, 2, 300]))),
        Err(iss(&[(".rgb[2]", "too_big")]))
    );
    // Serialize names and unknown or flattened names are not fields.
    assert_eq!(
        one("user_name", Some(json!("bob"))),
        Err(iss(&[(".user_name", "unrecognized_field")]))
    );
    assert_eq!(
        one("theme", Some(json!("dark"))),
        Err(iss(&[(".theme", "unrecognized_field")]))
    );

    let body = json!({"userName": "bob", "age": 30, "password": "secret-pw", "rgb": [0, 128, 255], "theme": "dark"});
    let w = Wizard::parse_value(&body).unwrap();
    assert_eq!(
        (w.rgb, w.extra.theme.as_str(), w.password.as_str()),
        ([0, 128, 255], "dark", "secret-pw")
    );
    let err = Wizard::parse_value(&json!({"userName": "bob", "rgb": [1, 2, 3, 4]})).unwrap_err();
    assert_eq!(
        issues(&err),
        iss(&[(".password", "missing_field"), (".rgb", "invalid_length")])
    );

    // serde writes neither `password` nor the empty `note`: validate still passes.
    let written = serde_json::to_value(&w).unwrap();
    assert!(written.get("password").is_none() && written.get("note").is_none());
    assert_eq!(Wizard::validate(&w).map_err(|e| issues(&e)), Ok(()));

    #[cfg(feature = "openapi")]
    {
        let js = Wizard::json_schema();
        let rgb = &js["properties"]["rgb"];
        assert_eq!(
            (rgb["minItems"].clone(), rgb["maxItems"].clone()),
            (json!(3), json!(3))
        );
    }
}
