use serde_json::{Value, json};
use vld::prelude::*;
use vld_fake::{FakeData, FakeGen, fake_value_seeded};

mod common;
use common::{fake_value, seed};

fn fake_many(schema: &Value, count: usize) -> Vec<Value> {
    (0..count).map(|_| fake_value(schema)).collect()
}

fn fake_parsed<T: VldParse>(schema: &Value) -> T {
    vld_fake::parse_or_panic(&fake_value(schema))
}

fn try_fake_parsed<T: VldParse>(schema: &Value) -> Result<T, VldError> {
    T::vld_parse_value(&fake_value(schema))
}

fn seeded_gen() -> FakeGen<rand::rngs::StdRng> {
    use rand::SeedableRng;
    FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(seed::next()))
}

// ───────────────────── test schemas ─────────────────────────────────────

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct SimpleUser {
        pub name:  String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age:   i64    => vld::number().int().min(18).max(99),
    }
}

vld_fake::impl_fake!(SimpleUser);

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Address {
        pub city: String => vld::string().min(1).max(100),
        pub zip:  String => vld::string().min(5).max(10),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct UserWithAddress {
        pub name:    String  => vld::string().min(2).max(50),
        pub address: Address => vld::nested(Address::parse_value),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Basic types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn string_basic_readable() {
    let schema = json!({"type": "string", "minLength": 3, "maxLength": 30});
    for _ in 0..50 {
        let val = fake_value(&schema);
        let s = val.as_str().expect("should be string");
        assert!(s.len() >= 3, "too short: {s}");
        assert!(s.len() <= 30, "too long: {s}");
        // Should be readable — mostly alphabetic with spaces
        assert!(
            s.chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-'),
            "non-readable chars: {s}"
        );
    }
}

#[test]
fn string_email_format() {
    let schema = json!({"type": "string", "format": "email"});
    for _ in 0..30 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(s.contains('@'), "missing @: {s}");
        assert!(s.contains('.'), "missing dot: {s}");
    }
}

#[test]
fn string_uuid_format() {
    let schema = json!({"type": "string", "format": "uuid"});
    for _ in 0..30 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert_eq!(s.len(), 36, "wrong uuid length: {s}");
        assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
    }
}

#[test]
fn string_url_format() {
    let schema = json!({"type": "string", "format": "url"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(
            s.starts_with("https://") || s.starts_with("http://"),
            "bad url: {s}"
        );
    }
}

#[test]
fn string_ipv4_format() {
    let schema = json!({"type": "string", "format": "ipv4"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        let parts: Vec<&str> = s.split('.').collect();
        assert_eq!(parts.len(), 4, "bad ipv4: {s}");
    }
}

#[test]
fn string_ipv6_format() {
    let schema = json!({"type": "string", "format": "ipv6"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        let groups: Vec<&str> = s.split(':').collect();
        assert_eq!(groups.len(), 8, "bad ipv6: {s}");
    }
}

#[test]
fn string_hostname_format() {
    let schema = json!({"type": "string", "format": "hostname"});
    let val = fake_value(&schema);
    let s = val.as_str().unwrap();
    assert!(s.contains('.'), "no dot: {s}");
}

#[test]
fn string_date_format() {
    let schema = json!({"type": "string", "format": "date"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert_eq!(s.len(), 10, "bad date: {s}");
        assert_eq!(s.chars().filter(|c| *c == '-').count(), 2);
    }
}

#[test]
fn string_datetime_format() {
    let schema = json!({"type": "string", "format": "date-time"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(s.contains('T'), "no T separator: {s}");
        assert!(s.ends_with('Z'), "no Z suffix: {s}");
    }
}

#[test]
fn string_base64_format() {
    let schema = json!({"type": "string", "format": "base64"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(s.len() >= 4, "too short base64: {s}");
    }
}

#[test]
fn string_ulid_format() {
    let schema = json!({"type": "string", "format": "ulid"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert_eq!(s.len(), 26, "bad ulid length: {s}");
    }
}

#[test]
fn string_emoji_format() {
    let schema = json!({"type": "string", "format": "emoji"});
    let val = fake_value(&schema);
    let s = val.as_str().unwrap();
    assert!(!s.is_empty(), "empty emoji");
}

#[test]
fn string_phone_format() {
    let schema = json!({"type": "string", "format": "phone"});
    let val = fake_value(&schema);
    let s = val.as_str().unwrap();
    assert!(s.starts_with('+'), "phone should start with +: {s}");
}

#[test]
fn string_slug_format() {
    let schema = json!({"type": "string", "format": "slug"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(s.contains('-'), "slug should have dashes: {s}");
        assert!(
            s.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "slug bad chars: {s}"
        );
    }
}

#[test]
fn string_mac_address_format() {
    let schema = json!({"type": "string", "format": "mac-address"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert_eq!(s.split(':').count(), 6, "bad mac: {s}");
    }
}

#[test]
fn string_credit_card_format() {
    let schema = json!({"type": "string", "format": "credit-card"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(s.starts_with('4'), "Visa starts with 4: {s}");
        assert_eq!(s.split('-').count(), 4, "bad cc format: {s}");
    }
}

#[test]
fn string_semver_format() {
    let schema = json!({"type": "string", "format": "semver"});
    for _ in 0..20 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert_eq!(s.split('.').count(), 3, "bad semver: {s}");
    }
}

#[test]
fn integer_basic() {
    let schema = json!({"type": "integer", "minimum": 10, "maximum": 20});
    for _ in 0..100 {
        let val = fake_value(&schema);
        let n = val.as_i64().expect("should be integer");
        assert!((10..=20).contains(&n), "out of range: {n}");
    }
}

#[test]
fn integer_exclusive() {
    let schema = json!({"type": "integer", "exclusiveMinimum": 0, "exclusiveMaximum": 5});
    for _ in 0..100 {
        let val = fake_value(&schema);
        let n = val.as_i64().unwrap();
        assert!((1..=4).contains(&n), "out of exclusive range: {n}");
    }
}

#[test]
fn integer_multiple_of() {
    let schema = json!({"type": "integer", "minimum": 0, "maximum": 100, "multipleOf": 5});
    for _ in 0..50 {
        let val = fake_value(&schema);
        let n = val.as_i64().unwrap();
        assert_eq!(n % 5, 0, "not multiple of 5: {n}");
    }
}

#[test]
fn number_float() {
    let schema = json!({"type": "number", "minimum": 1.0, "maximum": 10.0});
    for _ in 0..50 {
        let val = fake_value(&schema);
        let n = val.as_f64().expect("should be number");
        assert!((1.0..=10.0).contains(&n), "out of range: {n}");
    }
}

#[test]
fn boolean_type() {
    let schema = json!({"type": "boolean"});
    let mut seen_true = false;
    let mut seen_false = false;
    for _ in 0..100 {
        match fake_value(&schema) {
            Value::Bool(true) => seen_true = true,
            Value::Bool(false) => seen_false = true,
            other => panic!("unexpected: {other}"),
        }
    }
    assert!(
        seen_true && seen_false,
        "should produce both true and false"
    );
}

#[test]
fn null_type() {
    let schema = json!({"type": "null"});
    assert!(fake_value(&schema).is_null());
}

// ═══════════════════════════════════════════════════════════════════════════
//  Field-name heuristics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hint_name_is_realistic() {
    let schema = json!({
        "type": "object",
        "required": ["name", "email", "city", "phone"],
        "properties": {
            "name":  {"type": "string"},
            "email": {"type": "string"},
            "city":  {"type": "string"},
            "phone": {"type": "string"}
        }
    });
    for _ in 0..20 {
        let val = fake_value(&schema);
        let obj = val.as_object().unwrap();

        let name = obj["name"].as_str().unwrap();
        assert!(name.contains(' '), "name should be 'First Last': {name}");

        let email = obj["email"].as_str().unwrap();
        assert!(email.contains('@'), "email should contain @: {email}");

        let phone = obj["phone"].as_str().unwrap();
        assert!(phone.starts_with('+'), "phone should start with +: {phone}");
    }
}

#[test]
fn hint_username() {
    let schema = json!({
        "type": "object",
        "required": ["username"],
        "properties": {
            "username": {"type": "string"}
        }
    });
    for _ in 0..20 {
        let val = fake_value(&schema);
        let username = val["username"].as_str().unwrap();
        // Should be lowercase name + digits
        assert!(
            username
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "bad username: {username}"
        );
    }
}

#[test]
fn hint_company_realistic() {
    let schema = json!({
        "type": "object",
        "required": ["company", "department", "job_title"],
        "properties": {
            "company": {"type": "string"},
            "department": {"type": "string"},
            "job_title": {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();
    assert!(
        !obj["company"].as_str().unwrap().is_empty(),
        "company should not be empty"
    );
    assert!(
        !obj["department"].as_str().unwrap().is_empty(),
        "department should not be empty"
    );
    assert!(
        !obj["job_title"].as_str().unwrap().is_empty(),
        "job_title should not be empty"
    );
}

#[test]
fn hint_address_fields() {
    let schema = json!({
        "type": "object",
        "required": ["street", "city", "state", "country", "zip"],
        "properties": {
            "street":  {"type": "string"},
            "city":    {"type": "string"},
            "state":   {"type": "string"},
            "country": {"type": "string"},
            "zip":     {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();
    assert!(!obj["street"].as_str().unwrap().is_empty());
    assert!(!obj["city"].as_str().unwrap().is_empty());
    assert!(!obj["state"].as_str().unwrap().is_empty());
    assert!(!obj["country"].as_str().unwrap().is_empty());
    // zip should be 5 digits
    let zip = obj["zip"].as_str().unwrap();
    assert_eq!(zip.len(), 5, "zip should be 5 digits: {zip}");
}

#[test]
fn hint_product_fields() {
    let schema = json!({
        "type": "object",
        "required": ["product_name", "sku", "category", "color"],
        "properties": {
            "product_name": {"type": "string"},
            "sku":          {"type": "string"},
            "category":     {"type": "string"},
            "color":        {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();
    let sku = obj["sku"].as_str().unwrap();
    assert!(sku.contains('-'), "sku should be like ABC-12345: {sku}");
}

#[test]
fn hint_url_and_domain() {
    let schema = json!({
        "type": "object",
        "required": ["website", "hostname"],
        "properties": {
            "website":  {"type": "string"},
            "hostname": {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();
    let website = obj["website"].as_str().unwrap();
    assert!(
        website.starts_with("http"),
        "website should be URL: {website}"
    );
    let hostname = obj["hostname"].as_str().unwrap();
    assert!(
        hostname.contains('.'),
        "hostname should have dots: {hostname}"
    );
}

#[test]
fn hint_password_complex() {
    let schema = json!({
        "type": "object",
        "required": ["password"],
        "properties": {
            "password": {"type": "string"}
        }
    });
    for _ in 0..10 {
        let val = fake_value(&schema);
        let pwd = val["password"].as_str().unwrap();
        assert!(pwd.len() >= 10, "password too short: {pwd}");
    }
}

#[test]
fn hint_version_semver() {
    let schema = json!({
        "type": "object",
        "required": ["version"],
        "properties": {
            "version": {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let version = val["version"].as_str().unwrap();
    assert_eq!(
        version.split('.').count(),
        3,
        "version should be semver: {version}"
    );
}

#[test]
fn hint_description_is_sentence() {
    let schema = json!({
        "type": "object",
        "required": ["description"],
        "properties": {
            "description": {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let desc = val["description"].as_str().unwrap();
    assert!(desc.len() > 20, "description should be a sentence: {desc}");
    assert!(
        desc.ends_with('.'),
        "description should end with period: {desc}"
    );
}

#[test]
fn hint_id_is_uuid() {
    let schema = json!({
        "type": "object",
        "required": ["id"],
        "properties": {
            "id": {"type": "string"}
        }
    });
    let val = fake_value(&schema);
    let id = val["id"].as_str().unwrap();
    assert_eq!(id.len(), 36, "id should be UUID: {id}");
    assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Enum / const
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn enum_values() {
    let schema = json!({"enum": ["admin", "user", "moderator"]});
    for _ in 0..30 {
        let val = fake_value(&schema);
        let s = val.as_str().unwrap();
        assert!(
            ["admin", "user", "moderator"].contains(&s),
            "unexpected enum: {s}"
        );
    }
}

#[test]
fn const_value() {
    let schema = json!({"const": 42});
    assert_eq!(fake_value(&schema), json!(42));
}

// ═══════════════════════════════════════════════════════════════════════════
//  Array
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn array_basic() {
    let schema = json!({
        "type": "array",
        "items": {"type": "integer", "minimum": 0, "maximum": 100},
        "minItems": 2,
        "maxItems": 5
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let arr = val.as_array().unwrap();
        assert!(arr.len() >= 2 && arr.len() <= 5, "bad len: {}", arr.len());
        for item in arr {
            let n = item.as_i64().unwrap();
            assert!((0..=100).contains(&n));
        }
    }
}

#[test]
fn array_tuple_prefix_items() {
    let schema = json!({
        "type": "array",
        "prefixItems": [
            {"type": "string"},
            {"type": "integer"},
            {"type": "boolean"}
        ]
    });
    let val = fake_value(&schema);
    let arr = val.as_array().unwrap();
    assert!(arr.len() >= 3);
    assert!(arr[0].is_string());
    assert!(arr[1].is_i64() || arr[1].is_u64());
    assert!(arr[2].is_boolean());
}

#[test]
fn array_unique_items() {
    let schema = json!({
        "type": "array",
        "items": {"type": "integer", "minimum": 1, "maximum": 100},
        "minItems": 5,
        "maxItems": 5,
        "uniqueItems": true
    });
    for _ in 0..20 {
        let val = fake_value(&schema);
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 5);
        // Check uniqueness
        let mut seen = std::collections::HashSet::new();
        for item in arr {
            assert!(seen.insert(item.as_i64().unwrap()), "duplicate found");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Object
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn object_basic() {
    let schema = json!({
        "type": "object",
        "required": ["name", "age"],
        "properties": {
            "name": {"type": "string", "minLength": 2},
            "age":  {"type": "integer", "minimum": 0}
        }
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("name"), "missing name");
        assert!(obj.contains_key("age"), "missing age");
        assert!(obj["name"].is_string());
        assert!(obj["age"].is_i64() || obj["age"].is_u64());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  oneOf / allOf
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn one_of() {
    let schema = json!({
        "oneOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    let mut seen_str = false;
    let mut seen_int = false;
    for _ in 0..100 {
        let val = fake_value(&schema);
        if val.is_string() {
            seen_str = true;
        } else if val.is_i64() || val.is_u64() {
            seen_int = true;
        }
    }
    assert!(seen_str || seen_int, "should produce at least one variant");
}

#[test]
fn one_of_filters_null() {
    let schema = json!({
        "oneOf": [
            {"type": "string", "minLength": 1},
            {"type": "null"}
        ]
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        assert!(val.is_string(), "expected string, got: {val}");
    }
}

#[test]
fn all_of_merge() {
    let schema = json!({
        "allOf": [
            {"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]},
            {"type": "object", "properties": {"b": {"type": "integer"}}, "required": ["b"]}
        ]
    });
    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();
    assert!(obj.contains_key("a"));
    assert!(obj.contains_key("b"));
}

// ═══════════════════════════════════════════════════════════════════════════
//  vld schema integration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn simple_user_schema_roundtrip() {
    let schema = SimpleUser::json_schema();
    for _ in 0..20 {
        let val = fake_value(&schema);
        let parsed = SimpleUser::parse_value(&val);
        assert!(
            parsed.is_ok(),
            "Generated value did not pass validation:\nValue: {}\nError: {:?}",
            serde_json::to_string_pretty(&val).unwrap(),
            parsed.err()
        );
        let user = parsed.unwrap();
        assert!(user.name.len() >= 2);
        assert!(user.name.len() <= 50);
        assert!(user.email.contains('@'));
        assert!(user.age >= 18 && user.age <= 99);
    }
}

#[test]
fn nested_schema_roundtrip_raw() {
    let schema = json!({
        "type": "object",
        "required": ["name", "address"],
        "properties": {
            "name": {"type": "string", "minLength": 2, "maxLength": 50},
            "address": {
                "type": "object",
                "required": ["city", "zip"],
                "properties": {
                    "city": {"type": "string", "minLength": 1, "maxLength": 100},
                    "zip":  {"type": "string", "minLength": 5, "maxLength": 10}
                }
            }
        }
    });
    for _ in 0..20 {
        let val = fake_value(&schema);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("address"));
        let addr = obj["address"].as_object().unwrap();
        assert!(addr.contains_key("city"));
        assert!(addr.contains_key("zip"));
    }
}

#[test]
fn fake_parsed_typed() {
    let schema = SimpleUser::json_schema();
    for _ in 0..10 {
        let user: SimpleUser = fake_parsed(&schema);
        assert!(user.age >= 18 && user.age <= 99);
    }
}

#[test]
fn try_fake_parsed_ok() {
    let schema = SimpleUser::json_schema();
    for _ in 0..10 {
        let user = try_fake_parsed::<SimpleUser>(&schema).unwrap();
        assert!((2..=50).contains(&user.name.chars().count()), "{user:?}");
        assert!(user.email.contains('@'), "{user:?}");
        assert!((18..=99).contains(&user.age), "{user:?}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Convenience functions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn fake_many_count() {
    let schema = json!({"type": "integer", "minimum": 0, "maximum": 100});
    let values = fake_many(&schema, 10);
    assert_eq!(values.len(), 10);
    for v in &values {
        assert!(v.is_i64() || v.is_u64());
    }
}

#[test]
fn fake_json_returns_valid_json() {
    let schema = SimpleUser::json_schema();
    let json_str = vld_fake::fake_json(&schema);
    let parsed: Value = serde_json::from_str(&json_str).expect("should be valid JSON");
    assert!(parsed.is_object());
}

// ═══════════════════════════════════════════════════════════════════════════
//  Seeded / reproducible
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn seeded_reproducible() {
    let schema = SimpleUser::json_schema();
    let v1 = fake_value_seeded(&schema, 12345);
    let v2 = fake_value_seeded(&schema, 12345);
    assert_eq!(v1, v2, "same seed should produce same output");
}

#[test]
fn different_seeds_differ() {
    let schema = SimpleUser::json_schema();
    let v1 = fake_value_seeded(&schema, 11111);
    let v2 = fake_value_seeded(&schema, 22222);
    assert_ne!(v1, v2);
}

// ═══════════════════════════════════════════════════════════════════════════
//  FakeGen with custom rng
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn custom_rng() {
    use rand::SeedableRng;
    let rng = rand::rngs::StdRng::seed_from_u64(42);
    let mut generator = FakeGen::with_rng(rng);
    let schema = json!({"type": "string", "minLength": 5, "maxLength": 10});
    let val = generator.value(&schema);
    assert!(val.is_string());
    let s = val.as_str().unwrap();
    assert!(s.len() >= 5 && s.len() <= 10);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn empty_schema_returns_something() {
    let schema = json!({});
    let val = fake_value(&schema);
    assert!(val.is_string());
}

#[test]
fn unknown_type_returns_null() {
    let schema = json!({"type": "foobar"});
    assert!(fake_value(&schema).is_null());
}

#[test]
fn deeply_nested_stops_at_depth_limit() {
    fn deep_schema(depth: usize) -> Value {
        if depth == 0 {
            json!({"type": "string"})
        } else {
            json!({
                "type": "object",
                "required": ["child"],
                "properties": {
                    "child": deep_schema(depth - 1)
                }
            })
        }
    }
    /// Objects along the `child` chain, and the value where the chain ends.
    fn chain(mut v: &Value) -> (usize, &Value) {
        let mut objects = 0;
        while let Some(child) = v.get("child") {
            objects += 1;
            v = child;
        }
        (objects, v)
    }

    // Shallow schemas are generated completely.
    let full = fake_value(&deep_schema(5));
    let (objects, leaf) = chain(&full);
    assert_eq!(objects, 5, "{full}");
    assert!(leaf.is_string(), "{full}");

    // Recursion stops after 13 levels (MAX_DEPTH = 12): the 14th is null.
    let cut = fake_value(&deep_schema(20));
    let (objects, leaf) = chain(&cut);
    assert_eq!(objects, 13, "{cut}");
    assert!(leaf.is_null(), "{cut}");
}

// ═══════════════════════════════════════════════════════════════════════════
//  Complex realistic schema
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn realistic_user_profile() {
    let schema = json!({
        "type": "object",
        "required": [
            "id", "username", "email", "first_name", "last_name",
            "phone", "company", "job_title", "city", "country",
            "website", "bio", "tags"
        ],
        "properties": {
            "id":         {"type": "string"},
            "username":   {"type": "string"},
            "email":      {"type": "string"},
            "first_name": {"type": "string"},
            "last_name":  {"type": "string"},
            "phone":      {"type": "string"},
            "company":    {"type": "string"},
            "job_title":  {"type": "string"},
            "city":       {"type": "string"},
            "country":    {"type": "string"},
            "website":    {"type": "string"},
            "bio":        {"type": "string"},
            "tags":       {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 5}
        }
    });

    let val = fake_value(&schema);
    let obj = val.as_object().unwrap();

    // All required fields present
    assert_eq!(obj.len(), 13);

    // Verify realistic data from hints
    let id = obj["id"].as_str().unwrap();
    assert!(id.contains('-'), "id should be UUID-like: {id}");

    let email = obj["email"].as_str().unwrap();
    assert!(email.contains('@'), "email: {email}");

    let phone = obj["phone"].as_str().unwrap();
    assert!(phone.starts_with('+'), "phone: {phone}");

    let website = obj["website"].as_str().unwrap();
    assert!(website.starts_with("http"), "website: {website}");

    let bio = obj["bio"].as_str().unwrap();
    assert!(bio.ends_with('.'), "bio should be a sentence: {bio}");

    let tags = obj["tags"].as_array().unwrap();
    assert!(!tags.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
//  Object templates (empty schema + field hint)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn template_address_from_empty_object() {
    // This simulates vld::nested(Address::parse_value) which generates
    // {"type": "object"} without properties.
    let schema = json!({
        "type": "object",
        "required": ["name", "address"],
        "properties": {
            "name": {"type": "string"},
            "address": {"type": "object"}
        }
    });
    for _ in 0..10 {
        let val = fake_value(&schema);
        let addr = val["address"].as_object().unwrap();
        assert!(addr.contains_key("street"), "missing street: {addr:?}");
        assert!(addr.contains_key("city"), "missing city: {addr:?}");
        assert!(addr.contains_key("country"), "missing country: {addr:?}");
        assert!(addr.contains_key("zip"), "missing zip: {addr:?}");
        assert!(addr.contains_key("latitude"), "missing lat: {addr:?}");
        assert!(addr.contains_key("longitude"), "missing lng: {addr:?}");
    }
}

#[test]
fn template_location_geo() {
    let schema = json!({
        "type": "object",
        "required": ["location"],
        "properties": {
            "location": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let loc = val["location"].as_object().unwrap();
    assert!(loc.contains_key("latitude"));
    assert!(loc.contains_key("longitude"));
    let lat = loc["latitude"].as_f64().unwrap();
    let lng = loc["longitude"].as_f64().unwrap();
    assert!((-90.0..=90.0).contains(&lat), "bad lat: {lat}");
    assert!((-180.0..=180.0).contains(&lng), "bad lng: {lng}");
}

#[test]
fn template_person() {
    let schema = json!({
        "type": "object",
        "required": ["author"],
        "properties": {
            "author": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let person = val["author"].as_object().unwrap();
    assert!(person.contains_key("first_name"));
    assert!(person.contains_key("last_name"));
    assert!(person.contains_key("email"));
    assert!(person["email"].as_str().unwrap().contains('@'));
}

#[test]
fn template_company() {
    let schema = json!({
        "type": "object",
        "required": ["company"],
        "properties": {
            "company": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let co = val["company"].as_object().unwrap();
    assert!(co.contains_key("name"));
    assert!(co.contains_key("industry"));
    assert!(co.contains_key("employees"));
    assert!(co.contains_key("website"));
}

#[test]
fn template_product() {
    let schema = json!({
        "type": "object",
        "required": ["product"],
        "properties": {
            "product": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let prod = val["product"].as_object().unwrap();
    assert!(prod.contains_key("name"));
    assert!(prod.contains_key("sku"));
    assert!(prod.contains_key("price"));
    let price = prod["price"].as_f64().unwrap();
    assert!(price > 0.0, "negative price: {price}");
}

#[test]
fn template_image() {
    let schema = json!({
        "type": "object",
        "required": ["avatar"],
        "properties": {
            "avatar": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let img = val["avatar"].as_object().unwrap();
    assert!(img.contains_key("url"));
    assert!(img.contains_key("width"));
    assert!(img.contains_key("height"));
    assert!(img["url"].as_str().unwrap().starts_with("https://"));
}

#[test]
fn template_config() {
    let schema = json!({
        "type": "object",
        "required": ["settings"],
        "properties": {
            "settings": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let cfg = val["settings"].as_object().unwrap();
    assert!(cfg.contains_key("environment"));
    assert!(cfg.contains_key("port"));
    assert!(cfg.contains_key("host"));
}

#[test]
fn template_dimensions() {
    let schema = json!({
        "type": "object",
        "required": ["dimensions"],
        "properties": {
            "dimensions": {"type": "object"}
        }
    });
    let val = fake_value(&schema);
    let dim = val["dimensions"].as_object().unwrap();
    assert!(dim.contains_key("width"));
    assert!(dim.contains_key("height"));
    assert!(dim.contains_key("unit"));
}

// ═══════════════════════════════════════════════════════════════════════════
//  Number field-name heuristics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hint_number_latitude() {
    let schema = json!({
        "type": "object",
        "required": ["latitude", "longitude"],
        "properties": {
            "latitude":  {"type": "number"},
            "longitude": {"type": "number"}
        }
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let lat = val["latitude"].as_f64().unwrap();
        let lng = val["longitude"].as_f64().unwrap();
        assert!((-90.0..=90.0).contains(&lat), "bad lat: {lat}");
        assert!((-180.0..=180.0).contains(&lng), "bad lng: {lng}");
    }
}

#[test]
fn hint_number_price() {
    let schema = json!({
        "type": "object",
        "required": ["price"],
        "properties": {
            "price": {"type": "number"}
        }
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let price = val["price"].as_f64().unwrap();
        assert!(price >= 0.01, "price too low: {price}");
        assert!(price <= 9999.99, "price too high: {price}");
    }
}

#[test]
fn hint_number_temperature() {
    let schema = json!({
        "type": "object",
        "required": ["temperature"],
        "properties": {
            "temperature": {"type": "number"}
        }
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let temp = val["temperature"].as_f64().unwrap();
        assert!((-40.0..=50.0).contains(&temp), "bad temp: {temp}");
    }
}

#[test]
fn hint_number_rating() {
    let schema = json!({
        "type": "object",
        "required": ["rating"],
        "properties": {
            "rating": {"type": "number"}
        }
    });
    for _ in 0..30 {
        let val = fake_value(&schema);
        let rating = val["rating"].as_f64().unwrap();
        assert!((1.0..=5.0).contains(&rating), "bad rating: {rating}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  FakeData trait + impl_fake! macro (typed API)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn typed_fake_single() {
    let user = SimpleUser::fake();
    assert!(user.name.len() >= 2);
    assert!(user.name.len() <= 50);
    assert!(user.email.contains('@'));
    assert!(user.age >= 18 && user.age <= 99);
}

#[test]
fn typed_fake_many() {
    let users = SimpleUser::fake_many(10);
    assert_eq!(users.len(), 10);
    for u in &users {
        assert!(u.email.contains('@'));
        assert!(u.age >= 18 && u.age <= 99);
    }
}

#[test]
fn typed_fake_seeded_reproducible() {
    let u1 = SimpleUser::fake_seeded(42);
    let u2 = SimpleUser::fake_seeded(42);
    assert_eq!(u1.name, u2.name);
    assert_eq!(u1.email, u2.email);
    assert_eq!(u1.age, u2.age);
}

#[test]
fn typed_fake_seeded_different_seeds() {
    let u1 = SimpleUser::fake_seeded(111);
    let u2 = SimpleUser::fake_seeded(222);
    // Extremely unlikely to be equal
    assert_ne!(u1.name, u2.name);
}

#[test]
fn typed_try_fake_ok() {
    let result = SimpleUser::try_fake();
    assert!(result.is_ok());
    let user = result.unwrap();
    assert!(user.age >= 18);
}

// ═══════════════════════════════════════════════════════════════════════════
//  $ref resolution (vld::nested! emits `#/components/schemas/X`)
// ═══════════════════════════════════════════════════════════════════════════

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct RefAddress {
        pub city: String => vld::string().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct RefUser {
        pub name:    String     => vld::string().min(2).max(50),
        pub address: RefAddress => vld::nested!(RefAddress),
    }
}

vld_fake::impl_fake!(RefUser);

#[test]
fn typed_fake_resolves_nested_ref() {
    assert!(RefUser::json_schema()["properties"]["address"]["$ref"].is_string());
    for _ in 0..20 {
        let u = RefUser::fake_seeded(seed::next());
        assert!(!u.address.city.is_empty());
    }
    let _ = RefUser::fake_seeded(7);
    assert_eq!(RefUser::fake_many(3).len(), 3);
}

#[test]
fn raw_ref_resolves_defs_and_components() {
    let target = json!({"type": "object", "required": ["n"], "properties": {"n": {"type": "integer", "minimum": 1, "maximum": 3}}});
    for (root, r) in [
        ("$defs", "#/$defs/T"),
        ("definitions", "#/definitions/T"),
        ("$defs", "#/components/schemas/T"),
    ] {
        let mut schema =
            json!({"type": "object", "required": ["t"], "properties": {"t": {"$ref": r}}});
        schema[root] = json!({ "T": target });
        let v = fake_value(&schema);
        let n = v["t"]["n"].as_i64().expect("ref resolved to object");
        assert!((1..=3).contains(&n));
    }
    let doc = json!({
        "components": {"schemas": {"T": target}},
        "$ref": "#/components/schemas/T"
    });
    assert!(fake_value(&doc)["n"].is_i64());
}

#[test]
fn type_array_uses_non_null_type() {
    let schema = json!({"type": ["integer", "null"], "minimum": 5, "maximum": 6});
    for _ in 0..10 {
        let n = fake_value(&schema).as_i64().unwrap();
        assert!((5..=6).contains(&n));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Generated values must pass the very schema they were generated from
// ═══════════════════════════════════════════════════════════════════════════

fn assert_fakes<S>(s: S)
where
    S: vld::schema::VldSchema + vld::openapi::JsonSchema,
{
    let schema = s.json_schema();
    for _ in 0..200 {
        let v = fake_value(&schema);
        assert!(
            s.parse_value(&v).is_ok(),
            "schema {schema} produced invalid {v}: {:?}",
            s.parse_value(&v).err()
        );
    }
}

#[test]
fn every_core_string_format_is_generated_validly() {
    let s = vld::string;
    for schema in [
        s().email(),
        s().url(),
        s().url_strict(),
        s().uri(),
        s().uuid(),
        s().uuid_v1(),
        s().uuid_v4(),
        s().uuid_v7(),
        s().ip(),
        s().ipv4(),
        s().ipv6(),
        s().cidr(),
        s().mac(),
        s().hex(),
        s().slug(),
        s().color(),
        s().currency_code(),
        s().country_code(),
        s().locale(),
        s().cron(),
        s().credit_card(),
        s().phone(),
        s().phone_e164_strict(),
        s().semver(),
        s().semver_full(),
        s().jwt(),
        s().ascii(),
        s().alpha(),
        s().alphanumeric(),
        s().lowercase(),
        s().uppercase(),
        s().iso_date(),
        s().iso_datetime(),
        s().iso_time(),
        s().hostname(),
        s().cuid2(),
        s().ulid(),
        s().nanoid(),
        s().emoji(),
    ] {
        assert_fakes(schema);
    }
}

#[test]
fn free_length_formats_respect_min_max_length() {
    let s = vld::string;
    for schema in [
        s().hex().min(40).max(40),
        s().alpha().min(1).max(3),
        s().alphanumeric().min(50),
        s().lowercase().max(2),
        s().uppercase().min(30).max(31),
        s().email().min(40),
        s().ascii().min(3).max(4),
    ] {
        assert_fakes(schema);
    }
}

#[test]
fn non_string_formats_are_generated_validly() {
    assert_fakes(vld::bytes().base64().min_len(4).max_len(6));
    assert_fakes(vld::bytes().base64url());
    assert_fakes(vld::bytes().hex().len(3));
    assert_fakes(vld::decimal());
    assert_fakes(vld::ip_network());
    assert_fakes(vld::socket_addr());
    assert_fakes(vld::path());
}

// ═══════════════════════════════════════════════════════════════════════════
//  pattern
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pattern_is_honoured() {
    let schema =
        json!({"type": "string", "pattern": r"^(red|green)/[0-9a-f]{4}$", "maxLength": 10});
    let re = vld::regex_lite::Regex::new(r"^(red|green)/[0-9a-f]{4}$").unwrap();
    for _ in 0..100 {
        let v = fake_value(&schema);
        let s = v.as_str().unwrap();
        assert!(re.is_match(s) && s.len() <= 10, "{s}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Multi-byte strings: lengths count chars, never split a char
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hint_truncation_is_char_safe() {
    // CITIES contains "São Paulo"; max(2) used to split 'ã' and panic.
    let schema = json!({"type": "string", "maxLength": 2, "minLength": 2});
    let mut generator = seeded_gen();
    for _ in 0..2000 {
        let v = generator.value_with_hint(&schema, Some("city"));
        assert_eq!(v.as_str().unwrap().chars().count(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Numeric ranges: one-sided bounds, hints outside range, multipleOf
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integer_one_sided_and_hinted_ranges_never_panic() {
    let n = || vld::number().int();
    assert_fakes(n().min(5000)); // default max 1000 used to swap with min
    assert_fakes(n().max(-10)); // max without min
    assert_fakes(n().gt(7)); // exclusive only
    assert_fakes(n().multiple_of(7).min(5000));
    assert_fakes(n().multiple_of(5).max(-12));
    let mut generator = seeded_gen();
    for (hint, schema) in [
        ("year", json!({"type": "integer", "maximum": 1990})),
        ("age", json!({"type": "integer", "minimum": 150})),
        ("port", json!({"type": "integer", "maximum": 80})),
        ("count", json!({"type": "integer", "minimum": 500})),
    ] {
        let lo = schema["minimum"].as_i64().unwrap_or(i64::MIN);
        let hi = schema["maximum"].as_i64().unwrap_or(i64::MAX);
        for _ in 0..200 {
            let v = generator
                .value_with_hint(&schema, Some(hint))
                .as_i64()
                .unwrap();
            assert!(lo <= v && v <= hi, "{hint}: {v}");
        }
    }
    // Hints still narrow a half-open range.
    let year = generator.value_with_hint(&json!({"type": "integer", "minimum": 1}), Some("year"));
    assert!((2000..=2030).contains(&year.as_i64().unwrap()));
}

#[test]
fn float_multiple_of_and_tiny_ranges() {
    assert_fakes(vld::number().multiple_of(0.25).min(1.0).max(3.0));
    assert_fakes(vld::number().multiple_of(0.5));
    assert_fakes(vld::number().multiple_of(3.0).min(10.0).max(20.0));
    // Range narrower than the 2-decimal rounding step.
    assert_fakes(vld::number().min(0.001).max(0.002));
    assert_fakes(vld::number().gt(0.0).lt(0.001));
    assert_fakes(vld::number().max(-5.5));
    assert_fakes(vld::number().min(10_000.0));
}
