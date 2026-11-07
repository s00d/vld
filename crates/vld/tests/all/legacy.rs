//! Migration helpers: `blank_as`, `coerce_bools`, `clamp`, `VldType`,
//! `vld::de`, `#[vld(dynamic)]`.

use std::collections::HashMap;

use serde_json::json;
use vld::prelude::*;
use vld::schema::VldType;

#[test]
fn blank_as_maps_null_empty_and_missing() {
    let n = vld::number().int().coerce().blank_as(0);
    for input in ["null", r#""""#, r#"" ""#] {
        assert_eq!(n.parse(input).unwrap(), 0, "{input}");
    }
    assert_eq!(n.parse(r#""7""#).unwrap(), 7);
    assert!(n.parse(r#""x""#).is_err());
    let obj = vld::object().field("n", vld::number().int().blank_as(5));
    assert_eq!(obj.parse("{}").unwrap()["n"], json!(5));
    // with_default keeps rejecting blank strings.
    assert!(vld::number().int().with_default(0).parse(r#""""#).is_err());
}

#[test]
fn coerce_bools_writes_legacy_text() {
    let s = vld::string().coerce_bools("1", "0");
    assert_eq!(s.parse("true").unwrap(), "1");
    assert_eq!(s.parse("false").unwrap(), "0");
    assert_eq!(s.parse("12").unwrap(), "12");
    assert_eq!(vld::string().coerce().parse("true").unwrap(), "true");
}

#[test]
fn clamp_moves_into_inclusive_bounds() {
    let i = vld::number().int().min(0).max(100).clamp();
    assert_eq!(i.parse("1000").unwrap(), 100);
    assert_eq!(i.parse("-3").unwrap(), 0);
    assert_eq!(i.parse("50").unwrap(), 50);
    let f = vld::number().non_negative().max(1.5).clamp();
    assert_eq!(f.parse("-2").unwrap(), 0.0);
    assert_eq!(f.parse("9.5").unwrap(), 1.5);
    assert_eq!(vld::u8().clamp().parse("300").unwrap(), 255);
    assert_eq!(vld::i8().clamp().parse("-1000").unwrap(), -128);
    assert_eq!(vld::f32().clamp().parse("1e40").unwrap(), f32::MAX);
    // Exclusive bounds and other checks still fail.
    assert!(vld::number().int().gt(0).clamp().parse("0").is_err());
    assert!(vld::number().int().max(10).clamp().parse("1.5").is_err());
    // Without clamp nothing changes.
    assert!(vld::number().int().max(10).parse("11").is_err());
}

#[cfg(feature = "openapi")]
#[test]
fn clamp_and_blank_schemas() {
    use vld::json_schema::JsonSchema;
    let s = vld::u8().max(10).gt(1).clamp().json_schema();
    assert_eq!(s.get("maximum"), None);
    assert_eq!(s["exclusiveMinimum"], 1);
    let s = vld::number().int().blank_as(0).json_schema();
    assert_eq!(s["default"], 0);
    assert_eq!(s["anyOf"][1]["pattern"], "^\\s*$");
}

#[test]
fn vld_type_defaults() {
    assert_eq!(<Option<u16>>::vld_schema().parse("null").unwrap(), None);
    assert!(<u16>::vld_schema().parse("-1").is_err());
    assert!(<String>::vld_schema().parse("1").is_err());
    let m = <HashMap<String, Vec<bool>>>::vld_schema()
        .parse(r#"{"a": [true]}"#)
        .unwrap();
    assert_eq!(m["a"], [true]);
    assert_eq!(
        <serde_json::Value>::vld_schema().parse("[1]").unwrap(),
        json!([1])
    );
}

mod serde_bridge {
    use serde::Deserialize;

    vld::schema! {
        #[derive(Debug, Clone, PartialEq)]
        pub struct Inner {
            pub name: String => vld::string().min(2),
        }
    }

    vld::deserialize_fn!(fn percent -> i64 = vld::number().int().min(0).max(100).clamp());
    vld::deserialize_fn!(fn flag -> String = vld::string().coerce_bools("1", "0"));

    #[derive(Debug, Deserialize)]
    struct Settings {
        #[serde(deserialize_with = "percent")]
        volume: i64,
        #[serde(deserialize_with = "flag")]
        enabled: String,
        #[serde(deserialize_with = "vld::de::parse")]
        inner: Inner,
    }

    #[test]
    fn deserialize_with_runs_vld() {
        let s: Settings =
            serde_json::from_str(r#"{"volume": 140, "enabled": true, "inner": {"name": "ab"}}"#)
                .unwrap();
        assert_eq!(
            (s.volume, s.enabled.as_str(), s.inner.name.as_str()),
            (100, "1", "ab")
        );
        let err = serde_json::from_str::<Settings>(
            r#"{"volume": 1, "enabled": false, "inner": {"name": "a"}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("name"), "{err}");
    }
}

#[cfg(feature = "derive")]
mod derive {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    static LIMIT: AtomicI64 = AtomicI64::new(10);

    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    struct Address {
        city: String,
    }

    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    struct Profile {
        name: String,
        age: Option<u8>,
        tags: Vec<String>,
        #[vld(vld::string().email())]
        email: String,
        address: Address,
    }

    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    enum Shape {
        Circle(f64),
        Rect(u32, u32),
        Named { label: String },
    }

    #[test]
    fn fields_without_vld_use_their_type() {
        let ok = json!({
            "name": "x", "tags": ["a"], "email": "a@b.co", "address": {"city": "Riga"}
        });
        let p = Profile::parse_value(&ok).unwrap();
        assert_eq!((p.age, p.address.city.as_str()), (None, "Riga"));
        let bad = json!({"name": 1, "age": 300, "tags": "a", "email": "x", "address": {}});
        let e = Profile::parse_value(&bad).unwrap_err();
        let paths: Vec<String> = e.issues.iter().map(|i| i.path_string()).collect();
        assert_eq!(paths, [".name", ".age", ".tags", ".email", ".address.city"]);
        assert!(matches!(
            Shape::parse_value(&json!({"Rect": [1, 2]})),
            Ok(Shape::Rect(1, 2))
        ));
        assert!(Shape::parse_value(&json!({"Circle": "x"})).is_err());
        assert!(Shape::parse_value(&json!({"Named": {"label": "a"}})).is_ok());
    }

    #[cfg(feature = "openapi")]
    #[test]
    fn inferred_nested_type_is_a_ref() {
        let s = Profile::json_schema();
        assert_eq!(
            s["properties"]["address"]["$ref"],
            "#/components/schemas/Address"
        );
        assert_eq!(s["properties"]["age"]["format"], "uint8");
        let req = s["required"].as_array().unwrap();
        assert!(!req.contains(&json!("age")));
    }

    #[derive(Debug, vld::Validate)]
    #[vld(dynamic)]
    #[allow(dead_code)]
    struct Upload {
        #[vld(vld::number().int().max(LIMIT.load(Ordering::Relaxed)))]
        size: i64,
    }

    #[test]
    fn dynamic_rebuilds_schemas() {
        let v = json!({"size": 20});
        LIMIT.store(10, Ordering::Relaxed);
        assert!(Upload::parse_value(&v).is_err());
        LIMIT.store(50, Ordering::Relaxed);
        assert!(Upload::parse_value(&v).is_ok());
        #[cfg(feature = "openapi")]
        assert_eq!(Upload::json_schema()["properties"]["size"]["maximum"], 50);
    }
}
