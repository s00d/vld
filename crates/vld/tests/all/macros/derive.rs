//! `#[derive(Validate)]` (vld-derive).

use crate::common::{codes, paths};
use serde_json::json;
use vld::Validate;
use vld::error::IssueCode;
use vld::schema::VldParse;

#[derive(Debug, Validate)]
struct SimpleUser {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().min(0).optional())]
    age: Option<i64>,
}

#[test]
fn parses_and_reports_every_field() {
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
    assert_eq!(paths(&err), [".name", ".email", ".age"]);
    assert_eq!(codes(&err), ["too_small", "invalid_string", "too_small"]);
    for bad in [json!("str"), json!([1, 2])] {
        assert_issue!(SimpleUser::parse_value(&bad), "", IssueCode::InvalidType { expected, .. } if expected == "object");
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

#[test]
fn lenient_validate_fields_and_parse() {
    let fields = Config::validate_fields_value(&json!({"host": "", "port": 0})).unwrap();
    assert_eq!(
        fields.iter().map(|f| f.is_err()).collect::<Vec<_>>(),
        [true, true]
    );
    let r = Config::parse_lenient_value(&json!({"host": "h", "port": 0})).unwrap();
    assert_eq!(r.error_count(), 1);
    assert_eq!((r.value.host.as_str(), r.value.port), ("h", 0));
}

#[derive(Debug, serde::Serialize, Validate)]
struct CamelUser {
    #[serde(rename = "firstName")]
    #[vld(vld::string().min(2))]
    first_name: String,
}

#[test]
fn serde_rename_is_the_json_key() {
    let u = CamelUser::parse_value(&json!({"firstName": "John"})).unwrap();
    assert_eq!(u.first_name, "John");
    assert_eq!(
        paths(&CamelUser::parse_value(&json!({"first_name": "John"})).unwrap_err()),
        [".firstName"]
    );
}

/// No Default / Serialize: must still derive (lenient is opt-in).
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

#[test]
fn opaque_fields_and_raw_identifiers() {
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
        paths(&RawDerive::parse_value(&json!({})).unwrap_err()),
        [".type"]
    );
}

#[derive(Debug, serde::Deserialize, Validate)]
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

#[test]
fn serde_attributes_are_honoured() {
    let v = Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "skipped": "ignored"}))
        .unwrap();
    assert_eq!((v.first_name.as_str(), v.renamed.as_str()), ("a", "b"));
    assert!(v.skipped.is_empty());
    assert_eq!((v.defaulted, v.with_fn), (0, 7));
    // Present keys are still validated.
    let err =
        Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "DEFAULTED": 1})).unwrap_err();
    assert_eq!(paths(&err), [".DEFAULTED"]);
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

#[test]
fn generics_and_validate_helpers() {
    let p = Page::<Item>::parse_value(&json!({"items": [{"id": 1}]})).unwrap();
    assert_eq!(p.items[0].id, 1);
    assert_eq!(
        paths(&Page::<Item>::parse_value(&json!({"items": [{"id": 1}, {"id": "x"}]})).unwrap_err()),
        [".items[1].id"]
    );

    #[derive(serde::Serialize)]
    struct Raw {
        id: i64,
    }
    assert!(Item::is_valid(&Raw { id: 1 }));
    assert_issue!(
        Item::validate(&json!({"id": "x"})),
        ".id",
        IssueCode::InvalidType { .. }
    );
}

#[cfg(feature = "openapi")]
#[allow(dead_code)] // fields exist for their schemas
mod openapi {
    use super::*;

    #[derive(Debug, Validate)]
    pub struct DerivedAddr {
        #[vld(vld::string())]
        pub city: String,
    }

    #[derive(Debug, Validate)]
    pub struct DerivedUser {
        #[vld(vld::nested!(DerivedAddr))]
        pub addr: DerivedAddr,
        #[vld(vld::nested!(Page<Item>).optional())]
        pub page: Option<Page<Item>>,
    }

    #[test]
    fn nested_macro_registers_components() {
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

    #[test]
    fn serde_defaults_are_not_required() {
        let js = Serdeish::json_schema();
        assert_eq!(js["required"], json!(["FIRST_NAME", "de"]));
        assert!(js["properties"].get("skipped").is_none());
    }
}
