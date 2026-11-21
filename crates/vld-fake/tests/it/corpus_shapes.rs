//! Corpus: hand-written JSON Schemas (formats, numbers, enums, arrays,
//! objects, unions, field-name hints, object templates, `pattern`) with the
//! shape every seeded fake value must have.

use crate::common::{fake_value, seed};
use rand::SeedableRng;
use serde_json::{Value, json};
use vld_fake::FakeGen;

type Check = Box<dyn Fn(&Value) -> bool>;

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or(i64::MIN)
}

/// An object whose `hint` field is generated from an untyped `{"type": ty}`.
fn hinted(hint: &str, ty: &str) -> Value {
    json!({"type": "object", "required": [hint], "properties": {hint: {"type": ty}}})
}

fn passes<S: vld::schema::VldSchema>(schema: &S, v: &Value) -> bool {
    schema.parse_value(v).is_ok()
}

/// `978-…` / `979-…` with a valid ISBN-13 check digit.
fn isbn13(text: &str) -> bool {
    let d: Vec<u32> = text.chars().filter_map(|c| c.to_digit(10)).collect();
    let sum: u32 = d
        .iter()
        .enumerate()
        .map(|(i, x)| if i % 2 == 0 { *x } else { x * 3 })
        .sum();
    d.len() == 13 && (text.starts_with("978") || text.starts_with("979")) && sum % 10 == 0
}

fn pattern(p: &str) -> Value {
    json!({"type": "string", "pattern": p})
}

fn matches(p: &'static str) -> Check {
    let re = regex_lite::Regex::new(p).unwrap();
    Box::new(move |v| re.is_match(s(v)))
}

fn rows() -> Vec<(&'static str, Value, Check)> {
    let fmt = |f: &str| json!({"type": "string", "format": f});
    vec![
        // strings: readable text and formats
        (
            "text",
            json!({"type": "string", "minLength": 3, "maxLength": 30}),
            Box::new(|v| {
                let t = s(v);
                (3..=30).contains(&t.len())
                    && t.chars()
                        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
            }),
        ),
        (
            "email",
            fmt("email"),
            Box::new(|v| s(v).contains('@') && s(v).contains('.')),
        ),
        (
            "uuid",
            fmt("uuid"),
            Box::new(|v| s(v).len() == 36 && s(v).matches('-').count() == 4),
        ),
        (
            "url",
            fmt("url"),
            Box::new(|v| s(v).starts_with("https://") || s(v).starts_with("http://")),
        ),
        (
            "ipv4",
            fmt("ipv4"),
            Box::new(|v| s(v).split('.').count() == 4),
        ),
        (
            "ipv6",
            fmt("ipv6"),
            Box::new(|v| s(v).split(':').count() == 8),
        ),
        (
            "hostname",
            fmt("hostname"),
            Box::new(|v| s(v).contains('.')),
        ),
        (
            "date",
            fmt("date"),
            Box::new(|v| s(v).len() == 10 && s(v).matches('-').count() == 2),
        ),
        (
            "date-time",
            fmt("date-time"),
            Box::new(|v| s(v).contains('T') && s(v).ends_with('Z')),
        ),
        ("base64", fmt("base64"), Box::new(|v| s(v).len() >= 4)),
        ("ulid", fmt("ulid"), Box::new(|v| s(v).len() == 26)),
        ("emoji", fmt("emoji"), Box::new(|v| !s(v).is_empty())),
        ("phone", fmt("phone"), Box::new(|v| s(v).starts_with('+'))),
        (
            "slug",
            fmt("slug"),
            Box::new(|v| {
                s(v).contains('-') && s(v).chars().all(|c| c.is_ascii_lowercase() || c == '-')
            }),
        ),
        (
            "mac-address",
            fmt("mac-address"),
            Box::new(|v| s(v).split(':').count() == 6),
        ),
        (
            "credit-card",
            fmt("credit-card"),
            Box::new(|v| s(v).starts_with('4') && s(v).split('-').count() == 4),
        ),
        (
            "semver",
            fmt("semver"),
            Box::new(|v| s(v).split('.').count() == 3),
        ),
        (
            "pattern",
            json!({"type": "string", "pattern": r"^(red|green)/[0-9a-f]{4}$", "maxLength": 10}),
            Box::new(|v| {
                let re = regex_lite::Regex::new(r"^(red|green)/[0-9a-f]{4}$").unwrap();
                re.is_match(s(v)) && s(v).len() <= 10
            }),
        ),
        (
            "bad pattern",
            json!({"type": "string", "pattern": "(unclosed"}),
            Box::new(Value::is_string),
        ),
        (
            "pattern classes",
            pattern(r"^[A-Z]{3}-\d{4}$"),
            matches(r"^[A-Z]{3}-\d{4}$"),
        ),
        (
            "pattern alternation",
            pattern(r"^(foo|bar)+baz?$"),
            matches(r"^(foo|bar)+baz?$"),
        ),
        (
            "pattern words",
            pattern(r"^\w+@\w+\.com$"),
            matches(r"^\w+@\w+\.com$"),
        ),
        (
            "pattern negated",
            pattern(r"^[^a-z]{2,5}$"),
            matches(r"^[^a-z]{2,5}$"),
        ),
        (
            "pattern escapes",
            pattern(r"^a/b\.c$"),
            matches(r"^a/b\.c$"),
        ),
        (
            "pattern literal",
            pattern(r"^pre_"),
            Box::new(|v| s(v).starts_with("pre_")),
        ),
        (
            "pattern suffix",
            pattern(r"_suf$"),
            Box::new(|v| s(v).ends_with("_suf")),
        ),
        (
            "pattern contains",
            pattern(r"a\.b"),
            Box::new(|v| s(v).contains("a.b")),
        ),
        // check digits and encodings that must be right, not just look right
        (
            "credit-card luhn",
            fmt("credit-card"),
            Box::new(|v| passes(&vld::string().credit_card(), v)),
        ),
        (
            "card_number luhn",
            hinted("card_number", "string"),
            Box::new(|v| passes(&vld::string().credit_card(), &v["card_number"])),
        ),
        (
            "isbn checksum",
            hinted("isbn", "string"),
            Box::new(|v| isbn13(s(&v["isbn"]))),
        ),
        (
            "base64 decodes",
            fmt("base64"),
            Box::new(|v| passes(&vld::string().base64(), v)),
        ),
        (
            "byte decodes",
            fmt("byte"),
            Box::new(|v| passes(&vld::string().base64(), v)),
        ),
        (
            "byte 8",
            json!({"type": "string", "format": "byte", "minLength": 8, "maxLength": 8}),
            Box::new(|v| s(v).len() == 8 && passes(&vld::string().base64(), v)),
        ),
        // numbers
        (
            "int range",
            json!({"type": "integer", "minimum": 10, "maximum": 20}),
            Box::new(|v| (10..=20).contains(&i(v))),
        ),
        (
            "int exclusive",
            json!({"type": "integer", "exclusiveMinimum": 0, "exclusiveMaximum": 5}),
            Box::new(|v| (1..=4).contains(&i(v))),
        ),
        (
            "int multiple",
            json!({"type": "integer", "minimum": 0, "maximum": 100, "multipleOf": 5}),
            Box::new(|v| i(v) % 5 == 0 && (0..=100).contains(&i(v))),
        ),
        (
            "float range",
            json!({"type": "number", "minimum": 1.0, "maximum": 10.0}),
            Box::new(|v| (1.0..=10.0).contains(&f(v))),
        ),
        (
            "nullable int",
            json!({"type": ["integer", "null"], "minimum": 5, "maximum": 6}),
            Box::new(|v| (5..=6).contains(&i(v))),
        ),
        // scalars, enum, const
        ("null", json!({"type": "null"}), Box::new(Value::is_null)),
        (
            "unknown type",
            json!({"type": "foobar"}),
            Box::new(Value::is_null),
        ),
        ("empty schema", json!({}), Box::new(Value::is_string)),
        (
            "enum",
            json!({"enum": ["admin", "user", "moderator"]}),
            Box::new(|v| ["admin", "user", "moderator"].contains(&s(v))),
        ),
        ("const", json!({"const": 42}), Box::new(|v| *v == json!(42))),
        // arrays
        (
            "array",
            json!({"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 100}, "minItems": 2, "maxItems": 5}),
            Box::new(|v| {
                let a = v.as_array().unwrap();
                (2..=5).contains(&a.len()) && a.iter().all(|x| (0..=100).contains(&i(x)))
            }),
        ),
        (
            "tuple",
            json!({"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}, {"type": "boolean"}]}),
            Box::new(|v| {
                let a = v.as_array().unwrap();
                a.len() >= 3
                    && a[0].is_string()
                    && (a[1].is_i64() || a[1].is_u64())
                    && a[2].is_boolean()
            }),
        ),
        (
            "unique",
            json!({"type": "array", "items": {"type": "integer", "minimum": 1, "maximum": 100}, "minItems": 5, "maxItems": 5, "uniqueItems": true}),
            Box::new(|v| {
                let a = v.as_array().unwrap();
                let set: std::collections::HashSet<i64> = a.iter().map(i).collect();
                a.len() == 5 && set.len() == 5
            }),
        ),
        // objects and unions
        (
            "object",
            json!({"type": "object", "required": ["name", "age"], "properties": {"name": {"type": "string", "minLength": 2}, "age": {"type": "integer", "minimum": 0}}}),
            Box::new(|v| {
                v["name"].as_str().is_some_and(|n| n.chars().count() >= 2)
                    && v["age"].as_u64().is_some()
            }),
        ),
        (
            "nested object",
            json!({"type": "object", "required": ["name", "address"], "properties": {
            "name": {"type": "string", "minLength": 2, "maxLength": 50},
            "address": {"type": "object", "required": ["city", "zip"], "properties": {
                "city": {"type": "string", "minLength": 1, "maxLength": 100},
                "zip": {"type": "string", "minLength": 5, "maxLength": 10}}}}}),
            Box::new(|v| {
                v["name"].is_string()
                    && v["address"]["city"].is_string()
                    && v["address"]["zip"].is_string()
            }),
        ),
        (
            "oneOf",
            json!({"oneOf": [{"type": "string"}, {"type": "integer"}]}),
            Box::new(|v| v.is_string() || v.is_i64() || v.is_u64()),
        ),
        (
            "oneOf null",
            json!({"oneOf": [{"type": "string", "minLength": 1}, {"type": "null"}]}),
            Box::new(Value::is_string),
        ),
        (
            "allOf",
            json!({"allOf": [
            {"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]},
            {"type": "object", "properties": {"b": {"type": "integer"}}, "required": ["b"]}]}),
            Box::new(|v| v["a"].is_string() && v["b"].is_i64()),
        ),
        // field-name hints on untyped strings
        (
            "name",
            hinted("name", "string"),
            Box::new(|v| s(&v["name"]).contains(' ')),
        ),
        (
            "username",
            hinted("username", "string"),
            Box::new(|v| {
                s(&v["username"])
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            }),
        ),
        (
            "phone hint",
            hinted("phone", "string"),
            Box::new(|v| s(&v["phone"]).starts_with('+')),
        ),
        (
            "email hint",
            hinted("email", "string"),
            Box::new(|v| s(&v["email"]).contains('@')),
        ),
        (
            "company",
            hinted("company", "string"),
            Box::new(|v| !s(&v["company"]).is_empty()),
        ),
        (
            "department",
            hinted("department", "string"),
            Box::new(|v| !s(&v["department"]).is_empty()),
        ),
        (
            "job_title",
            hinted("job_title", "string"),
            Box::new(|v| !s(&v["job_title"]).is_empty()),
        ),
        (
            "street",
            hinted("street", "string"),
            Box::new(|v| !s(&v["street"]).is_empty()),
        ),
        (
            "city",
            hinted("city", "string"),
            Box::new(|v| !s(&v["city"]).is_empty()),
        ),
        (
            "state",
            hinted("state", "string"),
            Box::new(|v| !s(&v["state"]).is_empty()),
        ),
        (
            "country",
            hinted("country", "string"),
            Box::new(|v| !s(&v["country"]).is_empty()),
        ),
        (
            "zip",
            hinted("zip", "string"),
            Box::new(|v| s(&v["zip"]).len() == 5),
        ),
        (
            "sku",
            hinted("sku", "string"),
            Box::new(|v| s(&v["sku"]).contains('-')),
        ),
        (
            "website",
            hinted("website", "string"),
            Box::new(|v| s(&v["website"]).starts_with("http")),
        ),
        (
            "hostname hint",
            hinted("hostname", "string"),
            Box::new(|v| s(&v["hostname"]).contains('.')),
        ),
        (
            "password",
            hinted("password", "string"),
            Box::new(|v| s(&v["password"]).len() >= 10),
        ),
        (
            "version",
            hinted("version", "string"),
            Box::new(|v| s(&v["version"]).split('.').count() == 3),
        ),
        (
            "description",
            hinted("description", "string"),
            Box::new(|v| s(&v["description"]).len() > 20 && s(&v["description"]).ends_with('.')),
        ),
        (
            "bio",
            hinted("bio", "string"),
            Box::new(|v| s(&v["bio"]).ends_with('.')),
        ),
        (
            "id",
            hinted("id", "string"),
            Box::new(|v| s(&v["id"]).len() == 36 && s(&v["id"]).matches('-').count() == 4),
        ),
        // field-name hints on untyped numbers
        (
            "latitude",
            hinted("latitude", "number"),
            Box::new(|v| (-90.0..=90.0).contains(&f(&v["latitude"]))),
        ),
        (
            "longitude",
            hinted("longitude", "number"),
            Box::new(|v| (-180.0..=180.0).contains(&f(&v["longitude"]))),
        ),
        (
            "price",
            hinted("price", "number"),
            Box::new(|v| (0.01..=9999.99).contains(&f(&v["price"]))),
        ),
        (
            "temperature",
            hinted("temperature", "number"),
            Box::new(|v| (-40.0..=50.0).contains(&f(&v["temperature"]))),
        ),
        (
            "rating",
            hinted("rating", "number"),
            Box::new(|v| (1.0..=5.0).contains(&f(&v["rating"]))),
        ),
        // object templates: an empty object schema plus a field-name hint
        (
            "address",
            hinted("address", "object"),
            Box::new(|v| {
                ["street", "city", "country", "zip", "latitude", "longitude"]
                    .iter()
                    .all(|k| v["address"].get(k).is_some())
            }),
        ),
        (
            "location",
            hinted("location", "object"),
            Box::new(|v| {
                (-90.0..=90.0).contains(&f(&v["location"]["latitude"]))
                    && (-180.0..=180.0).contains(&f(&v["location"]["longitude"]))
            }),
        ),
        (
            "author",
            hinted("author", "object"),
            Box::new(|v| {
                v["author"]["first_name"].is_string()
                    && v["author"]["last_name"].is_string()
                    && s(&v["author"]["email"]).contains('@')
            }),
        ),
        (
            "company template",
            hinted("company", "object"),
            Box::new(|v| {
                ["name", "industry", "employees", "website"]
                    .iter()
                    .all(|k| v["company"].get(k).is_some())
            }),
        ),
        (
            "product",
            hinted("product", "object"),
            Box::new(|v| {
                v["product"]["name"].is_string()
                    && v["product"]["sku"].is_string()
                    && f(&v["product"]["price"]) > 0.0
            }),
        ),
        (
            "avatar",
            hinted("avatar", "object"),
            Box::new(|v| {
                s(&v["avatar"]["url"]).starts_with("https://")
                    && v["avatar"]["width"].is_number()
                    && v["avatar"]["height"].is_number()
            }),
        ),
        (
            "settings",
            hinted("settings", "object"),
            Box::new(|v| {
                ["environment", "port", "host"]
                    .iter()
                    .all(|k| v["settings"].get(k).is_some())
            }),
        ),
        (
            "dimensions",
            hinted("dimensions", "object"),
            Box::new(|v| {
                ["width", "height", "unit"]
                    .iter()
                    .all(|k| v["dimensions"].get(k).is_some())
            }),
        ),
    ]
}

/// Given each row's schema, when 30 seeded fakes are generated, then every
/// one has the row's shape.
#[test]
fn fakes_have_the_expected_shape() {
    let mut failures = Vec::new();
    for (name, schema, ok) in rows() {
        for _ in 0..30 {
            let v = fake_value(&schema);
            if !ok(&v) {
                failures.push(format!("{name}: {v}"));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Given a boolean and a two-branch union, when many fakes are generated,
/// then both values / branches show up.
#[test]
fn choices_cover_every_option() {
    let bools: Vec<Value> = (0..100)
        .map(|_| fake_value(&json!({"type": "boolean"})))
        .collect();
    assert!(bools.contains(&json!(true)) && bools.contains(&json!(false)));
    // (A bare `{"type": "string"}` branch gives way to the others: see the
    // "oneOf" row.)
    let union = json!({"oneOf": [{"type": "string", "minLength": 1}, {"type": "integer"}]});
    let kinds: std::collections::HashSet<bool> =
        (0..100).map(|_| fake_value(&union).is_string()).collect();
    assert_eq!(kinds.len(), 2, "both branches");
}

/// Given hints whose usual range lies outside the schema's bounds, and a
/// multi-byte dictionary word cut to two chars, when values are generated
/// with the hint, then the schema's bounds win, a half-open range is still
/// narrowed by the hint, and strings are cut on char boundaries.
#[test]
fn hints_never_break_the_schema() {
    let mut generator = FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(seed::next()));
    for (hint, schema) in [
        ("year", json!({"type": "integer", "maximum": 1990})),
        ("age", json!({"type": "integer", "minimum": 150})),
        ("port", json!({"type": "integer", "maximum": 80})),
        ("count", json!({"type": "integer", "minimum": 500})),
    ] {
        let lo = schema["minimum"].as_i64().unwrap_or(i64::MIN);
        let hi = schema["maximum"].as_i64().unwrap_or(i64::MAX);
        for _ in 0..200 {
            let v = i(&generator.value_with_hint(&schema, Some(hint)));
            assert!(lo <= v && v <= hi, "{hint}: {v}");
        }
    }
    let year = generator.value_with_hint(&json!({"type": "integer", "minimum": 1}), Some("year"));
    assert!((2000..=2030).contains(&i(&year)), "{year}");

    // CITIES contains "São Paulo"; max(2) used to split 'ã' and panic.
    let two = json!({"type": "string", "maxLength": 2, "minLength": 2});
    for _ in 0..2000 {
        let v = generator.value_with_hint(&two, Some("city"));
        assert_eq!(s(&v).chars().count(), 2, "{v}");
    }
}
