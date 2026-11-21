//! Stories: moving a legacy settings loader onto vld — blank values (`blank_as`), booleans
//! stored as text (`coerce_bools`), out-of-range numbers (`clamp`), schemas inferred from Rust
//! types (`VldType`, derive without `#[vld]`), serde integration (`vld::de::parse`,
//! `deserialize_fn!`) and limits read at parse time (`#[vld(dynamic)]`).
//!
//! The random combinations of these options are the models (`model_schema.rs`: `blank_as`,
//! `clamp`, `coerce`; `model_numbers.rs`: `clamp` on every number type; `model_strings.rs`:
//! `coerce_bools`); these stories walk one migration end to end.

use crate::common::*;

use serde_json::json;
use std::collections::HashMap;
use vld::prelude::*;
use vld::schema::VldType;

/// Given a legacy config where "not set" was written as `null`, `""` or `" "`, booleans were
/// written to text fields as `"1"` / `"0"`, and numbers were silently clamped
/// When the new loader reads it with `blank_as`, `coerce_bools` and `clamp`
/// Then blanks and absent keys become the fallback (where `with_default` still rejects blanks),
/// booleans become the legacy text, numbers move onto the inclusive bounds and the type range,
/// while exclusive bounds and non-integers still fail.
#[test]
fn legacy_values_load_into_the_new_schema() {
    let n = vld::number().int().coerce().blank_as(0);
    for input in ["null", r#""""#, r#"" ""#] {
        assert_eq!(n.parse(input).unwrap(), 0, "{input}");
    }
    assert_eq!(n.parse(r#""7""#).unwrap(), 7);
    assert_eq!(
        issues(&n.parse(r#""x""#).unwrap_err()),
        [(String::new(), "invalid_type".into())]
    );
    let obj = vld::object().field("n", vld::number().int().blank_as(5));
    assert_eq!(obj.parse("{}").unwrap()["n"], json!(5));
    assert!(vld::number().int().with_default(0).parse(r#""""#).is_err());

    let s = vld::string().coerce_bools("1", "0");
    assert_eq!(s.parse("true").unwrap(), "1");
    assert_eq!(s.parse("false").unwrap(), "0");
    assert_eq!(s.parse("12").unwrap(), "12");
    assert_eq!(vld::string().coerce().parse("true").unwrap(), "true");

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
    let code = |r: Result<i64, VldError>| issues(&r.unwrap_err());
    assert_eq!(
        code(vld::number().int().gt(0).clamp().parse("0")),
        [(String::new(), "too_small".into())]
    );
    assert_eq!(
        code(vld::number().int().max(10).clamp().parse("1.5")),
        [(String::new(), "not_int".into())]
    );
    assert_eq!(
        code(vld::number().int().max(10).parse("11")),
        [(String::new(), "too_big".into())]
    );

    #[cfg(feature = "openapi")]
    {
        use vld::json_schema::JsonSchema;
        // A clamped bound is no constraint for clients; exclusive ones stay.
        let s = vld::u8().max(10).gt(1).clamp().json_schema();
        assert_eq!(s.get("maximum"), None);
        assert_eq!(s["exclusiveMinimum"], 1);
        let s = vld::number().int().blank_as(0).json_schema();
        assert_eq!(s["default"], 0);
        assert_eq!(s["anyOf"][1]["pattern"], "^\\s*$");
    }
}

/// Given settings typed as plain Rust types (`Option<u16>`, `u16`, `String`,
/// `HashMap<String, Vec<bool>>`, `serde_json::Value`)
/// When their schemas come from `VldType::vld_schema()`
/// Then each schema accepts exactly what the type can hold.
#[test]
fn schemas_inferred_from_rust_types() {
    assert_eq!(<Option<u16>>::vld_schema().parse("null").unwrap(), None);
    assert_eq!(
        issues(&<u16>::vld_schema().parse("-1").unwrap_err()),
        [(String::new(), "too_small".into())]
    );
    assert_eq!(
        issues(&<String>::vld_schema().parse("1").unwrap_err()),
        [(String::new(), "invalid_type".into())]
    );
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

    /// Given an existing `#[derive(Deserialize)]` settings struct
    /// When some fields switch to `deserialize_with` functions made by `deserialize_fn!` or to
    /// `vld::de::parse` for a vld schema type
    /// Then `serde_json::from_str` runs the vld schemas (clamp, legacy booleans, nested checks)
    /// and a vld failure becomes the serde error, naming the field.
    #[test]
    fn serde_structs_run_vld_through_deserialize_with() {
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

    #[derive(Debug, vld::Validate)]
    #[vld(dynamic)]
    #[allow(dead_code)]
    struct Upload {
        #[vld(vld::number().int().max(LIMIT.load(Ordering::Relaxed)))]
        size: i64,
    }

    /// Given the old settings structs and enums derived with `Validate`, most fields without a
    /// `#[vld]` attribute, and an upload limit read from a runtime value (`#[vld(dynamic)]`)
    /// When documents are parsed and the schema is exported
    /// Then untagged fields are validated by their type (nested structs, `Option`, `Vec`, `u8`
    /// range), every issue path is reported, externally tagged enum variants parse, a nested type
    /// is a `$ref` with `Option` fields not required, and the dynamic limit is re-read on every
    /// parse and schema export.
    #[test]
    fn derived_settings_infer_field_schemas_and_read_live_limits() {
        let ok =
            json!({"name": "x", "tags": ["a"], "email": "a@b.co", "address": {"city": "Riga"}});
        let p = Profile::parse_value(&ok).unwrap();
        assert_eq!((p.age, p.address.city.as_str()), (None, "Riga"));
        let bad = json!({"name": 1, "age": 300, "tags": "a", "email": "x", "address": {}});
        let e = Profile::parse_value(&bad).unwrap_err();
        assert_eq!(
            issues(&e),
            [
                (".name".into(), "invalid_type".into()),
                (".age".into(), "too_big".into()),
                (".tags".into(), "invalid_type".into()),
                (".email".into(), "invalid_string".into()),
                (".address.city".into(), "missing_field".into()),
            ]
        );
        assert!(matches!(
            Shape::parse_value(&json!({"Rect": [1, 2]})),
            Ok(Shape::Rect(1, 2))
        ));
        assert!(Shape::parse_value(&json!({"Circle": "x"})).is_err());
        assert!(Shape::parse_value(&json!({"Named": {"label": "a"}})).is_ok());

        #[cfg(feature = "openapi")]
        {
            let s = Profile::json_schema();
            assert_eq!(
                s["properties"]["address"]["$ref"],
                "#/components/schemas/Address"
            );
            assert_eq!(s["properties"]["age"]["format"], "uint8");
            assert!(!s["required"].as_array().unwrap().contains(&json!("age")));
        }

        let v = json!({"size": 20});
        LIMIT.store(10, Ordering::Relaxed);
        assert_eq!(
            issues(&Upload::parse_value(&v).unwrap_err()),
            [(".size".into(), "too_big".into())]
        );
        LIMIT.store(50, Ordering::Relaxed);
        assert!(Upload::parse_value(&v).is_ok());
        #[cfg(feature = "openapi")]
        assert_eq!(Upload::json_schema()["properties"]["size"]["maximum"], 50);
    }
}
