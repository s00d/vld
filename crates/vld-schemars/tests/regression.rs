use serde_json::json;
use vld_schemars::schemars;
use vld_schemars::*;

mod common;
use common::{invalid, valid};

vld::schema! {
    #[derive(Debug)]
    pub struct RegAddr {
        pub city: String => vld::string().min(1),
        pub zip: String  => vld::string().min(5).max(10),
    }
}
impl_json_schema!(RegAddr);

vld::schema! {
    #[derive(Debug)]
    pub struct RegCompany {
        pub name: String => vld::string().min(1),
        pub hq: RegAddr => vld::nested!(RegAddr),
    }
}
impl_json_schema!(RegCompany);

#[test]
fn standalone_root_schema_has_resolvable_refs() {
    let root = generate_from_schemars::<RegCompany>();
    let r = root["properties"]["hq"]["$ref"].as_str().unwrap();
    let ptr = r.strip_prefix('#').unwrap();
    assert!(root.pointer(ptr).is_some(), "dangling ref {r} in {root}");
    // and the validator follows it
    let ok = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "75001"}});
    let bad = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "1"}});
    valid(&root, &ok);
    invalid(&root, &bad, &[(".hq.zip", "too_small")]);
}

#[test]
fn openapi3_generator_keeps_components_path() {
    let mut generator = schemars::generate::SchemaSettings::openapi3().into_generator();
    let schema = <RegCompany as schemars::JsonSchema>::json_schema(&mut generator);
    assert_eq!(
        schema.get("properties").unwrap()["hq"]["$ref"],
        "#/components/schemas/RegAddr"
    );
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Inner {
    #[schemars(length(min = 3))]
    code: String,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Outer {
    inner: Inner,
    pair: (u8, String),
}

#[test]
fn validator_follows_ref_to_nested_struct() {
    let schema = generate_from_schemars::<Outer>();
    let ok = json!({"inner": {"code": "abc"}, "pair": [1, "x"]});
    let bad = json!({"inner": {"code": "a"}, "pair": [1, "x"]});
    valid(&schema, &ok);
    invalid(&schema, &bad, &[(".inner.code", "too_small")]);
}

#[test]
fn validator_enforces_additional_properties_false() {
    let schema = generate_from_schemars::<Outer>();
    let extra = json!({"inner": {"code": "abc"}, "pair": [1, "x"], "extra": 1});
    invalid(&schema, &extra, &[(".extra", "unrecognized_field")]);
}

#[test]
fn validator_additional_properties_schema() {
    let schema = json!({"type": "object", "additionalProperties": {"type": "integer"}});
    valid(&schema, &json!({"a": 1}));
    invalid(&schema, &json!({"a": "x"}), &[(".a", "invalid_type")]);
}

#[test]
fn validator_checks_prefix_items() {
    let schema = generate_from_schemars::<Outer>();
    let bad = json!({"inner": {"code": "abc"}, "pair": ["no", "x"]});
    invalid(&schema, &bad, &[(".pair[0]", "invalid_type")]);
    let raw = json!({"prefixItems": [{"type": "integer"}], "items": {"type": "string"}});
    valid(&raw, &json!([1, "a", "b"]));
    invalid(&raw, &json!([1, 2]), &[("[1]", "invalid_type")]);
}

#[test]
fn validator_string_length_counts_chars() {
    let schema = json!({"type": "string", "maxLength": 3, "minLength": 3});
    valid(&schema, &json!("äöü"));
    invalid(&schema, &json!("äö"), &[("", "too_small")]);
    invalid(&schema, &json!("äöüß"), &[("", "too_big")]);
}

#[test]
fn validator_multiple_of_float_tolerance() {
    let schema = json!({"type": "number", "multipleOf": 0.1});
    valid(&schema, &json!(0.3));
    invalid(&schema, &json!(0.35), &[("", "multipleOf")]);
}

#[test]
fn validator_invalid_pattern_is_error() {
    let schema = json!({"type": "string", "pattern": "(unclosed"});
    let err = validate_with_schema(&schema, &json!("x")).unwrap_err();
    assert!(err.issues[0].message.contains("Invalid pattern"));
    invalid(&schema, &json!("x"), &[("", "invalid_pattern")]);
}

#[test]
fn validator_uri_accepts_any_scheme() {
    let schema = json!({"type": "string", "format": "uri"});
    for ok in ["mailto:a@b.c", "urn:isbn:123", "ftp://x.y", "https://x.y"] {
        valid(&schema, &json!(ok));
    }
    for bad in ["no-scheme", "1http://x", "http://a b"] {
        invalid(&schema, &json!(bad), &[("", "invalid_string")]);
    }
}

#[test]
fn validator_any_of_union() {
    let schema = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});
    valid(&schema, &json!(null));
    invalid(&schema, &json!(1), &[("", "anyOf")]);
}

#[test]
fn validator_self_ref_does_not_overflow() {
    invalid(&json!({"$ref": "#"}), &json!(1), &[("", "ref_depth")]);
    let dangling = json!({"$ref": "#/$defs/Missing"});
    invalid(&dangling, &json!(1), &[("", "invalid_ref")]);
}

// ---------------------------------------------------------------------------
// 0.5.0 re-audit
// ---------------------------------------------------------------------------

#[test]
fn openapi3_exclusive_bounds_are_booleans() {
    let mut s = json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10});
    to_openapi3(&mut s);
    assert_eq!(
        s,
        json!({"type": "number", "minimum": 0, "exclusiveMinimum": true, "maximum": 10, "exclusiveMaximum": true})
    );
    // A stricter inclusive bound wins; a looser one is replaced.
    let mut s = json!({"type": "integer", "minimum": 5, "exclusiveMinimum": 0, "maximum": 20, "exclusiveMaximum": 10});
    to_openapi3(&mut s);
    assert_eq!(s["minimum"], 5);
    assert!(s.get("exclusiveMinimum").is_none(), "{s}");
    assert_eq!(s["maximum"], 10);
    assert_eq!(s["exclusiveMaximum"], true);
    // From a real vld schema.
    let mut n = vld::openapi::JsonSchema::json_schema(&vld::number().gt(1.5));
    to_openapi3(&mut n);
    assert_eq!(n["minimum"], 1.5, "{n}");
    assert_eq!(n["exclusiveMinimum"], true, "{n}");
}

#[test]
fn openapi3_null_has_a_type() {
    let mut s = vld::openapi::JsonSchema::json_schema(&vld::null());
    to_openapi3(&mut s);
    assert_eq!(
        s,
        json!({"type": "string", "nullable": true, "enum": [null]})
    );
    // Idempotent.
    let before = s.clone();
    to_openapi3(&mut s);
    assert_eq!(s, before);
}

#[test]
fn invalid_schema_value_fails_closed() {
    assert!(try_vld_to_schemars(&json!(42)).is_err());
    assert_eq!(vld_to_schemars(&json!("x")).as_bool(), Some(false));
    assert_eq!(
        vld_to_schemars(&json!({"type": "string"}))
            .get("type")
            .unwrap(),
        "string"
    );
}

#[test]
fn validator_contains_counts() {
    let schema = vld::openapi::JsonSchema::json_schema(
        &vld::array(vld::number())
            .contains(3)
            .min_contains(2)
            .max_contains(3),
    );
    for (v, ok) in [
        (json!([3, 3]), true),
        (json!([3, 3, 3, 1]), true),
        (json!([3]), false),
        (json!([3, 3, 3, 3]), false),
    ] {
        assert_eq!(validate_with_schema(&schema, &v).is_ok(), ok, "{v}");
    }
    // Default minContains is 1.
    let s = json!({"contains": {"const": "x"}});
    valid(&s, &json!(["x"]));
    invalid(&s, &json!([]), &[("", "contains")]);
}

#[test]
fn validator_property_names() {
    let schema = vld::openapi::JsonSchema::json_schema(&vld::record_with_keys(
        vld::string().min(2),
        vld::number(),
    ));
    valid(&schema, &json!({"ab": 1}));
    let err = validate_with_schema(&schema, &json!({"a": 1})).unwrap_err();
    assert_eq!(
        err.issues[0].path,
        vec![vld::error::PathSegment::Field("a".into())]
    );
}

#[test]
fn validator_format_bounds() {
    let d = json!({"type": "string", "format": "date", "formatMinimum": "2020-01-01", "formatMaximum": "2020-12-31"});
    valid(&d, &json!("2020-06-01"));
    invalid(&d, &json!("2019-12-31"), &[("", "too_small")]);
    invalid(&d, &json!("2021-01-01"), &[("", "too_big")]);
    let dt =
        json!({"type": "string", "format": "date-time", "formatMinimum": "2020-01-01T10:00:00Z"});
    // 12:00+03:00 is 09:00Z: too early, although lexically larger.
    invalid(
        &dt,
        &json!("2020-01-01T12:00:00+03:00"),
        &[("", "too_small")],
    );
    valid(&dt, &json!("2020-01-01T10:00:00Z"));
    valid(&dt, &json!("2020-01-01T08:00:00-03:00"));
    let t = json!({"type": "string", "format": "time", "formatMaximum": "10:00:00"});
    valid(&t, &json!("10:00"));
    invalid(&t, &json!("10:00:00.5"), &[("", "too_big")]);
}

#[test]
fn validator_compares_large_integers_exactly() {
    let s = json!({"type": "integer", "maximum": 9007199254740992u64});
    // 2^53 + 1 rounds to 2^53 as f64.
    invalid(&s, &json!(9007199254740993u64), &[("", "too_big")]);
    let s = json!({"type": "integer", "minimum": u64::MAX});
    invalid(&s, &json!(u64::MAX - 1), &[("", "too_small")]);
    valid(&s, &json!(u64::MAX));
    let m = json!({"multipleOf": 3});
    // 2^53 + 3 rounds to 2^53 + 4 (a multiple of 3) as f64.
    invalid(&m, &json!(9007199254740995u64), &[("", "multipleOf")]);
    valid(&m, &json!(9007199254740993u64));
}

#[test]
fn validator_email_matches_vld() {
    let s = json!({"type": "string", "format": "email"});
    let email = vld::string().email();
    assert_eq!(
        vld::schema::VldSchema::parse_value(&email, &json!("a@b")).unwrap(),
        "a@b"
    );
    valid(&s, &json!("a@b"));
    for bad in ["ab", "@b"] {
        assert!(vld::schema::VldSchema::parse_value(&email, &json!(bad)).is_err());
        invalid(&s, &json!(bad), &[("", "invalid_string")]);
    }
}
