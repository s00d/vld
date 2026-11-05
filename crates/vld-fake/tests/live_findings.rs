//! Regression tests for the live end-to-end findings (0.5.0): 1000 fake
//! values per schema must all pass the schema they were generated from.

use vld::json_schema::JsonSchema;
use vld::schema::VldSchema;

fn check<S: VldSchema + JsonSchema>(name: &str, schema: S) {
    let js = schema.json_schema();
    for _ in 0..1000 {
        let v = vld_fake::fake_value(&js);
        if let Err(e) = schema.parse_value(&v) {
            panic!("{name}: {v} -> {e} (schema {js})");
        }
    }
}

fn re(s: &str) -> vld::regex_lite::Regex {
    vld::regex_lite::Regex::new(s).unwrap()
}

#[test]
fn string_formats() {
    check("base64url", vld::string().base64url());
    check("base64url_len", vld::string().base64url().len(8));
    check("json", vld::string().json());
    check("email_max", vld::string().email().max(20));
}

#[test]
fn numbers() {
    check("negative", vld::number().negative());
    check("negative_int", vld::number().int().negative());
    check("u64_top", vld::u64().min(u64::MAX - 5));
    check("i64_bottom", vld::i64().min(i64::MIN).max(i64::MIN + 3));
}

#[test]
fn records() {
    check(
        "record_min_keys",
        vld::record(vld::u8().max(3)).min_keys(1).max_keys(4),
    );
    check(
        "record_with_keys",
        vld::record_with_keys(vld::string().regex(re("^k[0-9]$")), vld::number().int()),
    );
}

#[test]
fn duration_and_patterns() {
    check("duration", vld::duration());
    check(
        "starts_ends",
        vld::string().starts_with("pre_").ends_with("_suf").min(10),
    );
    check("contains_max", vld::string().contains("@@").max(6));
    check(
        "regex_and_prefix",
        vld::string()
            .regex(re("^[a-z]{3}[0-9]{2}$"))
            .starts_with("ab"),
    );
}

// ---------------------------------------------------------------------------
// 0.5.0 re-audit: mass check (300 values per schema, 0 failures).
// ---------------------------------------------------------------------------

/// Like [`check`], with `patch` merged into the generated JSON Schema (the
/// keywords the 0.5 core emits, e.g. `x-past`, `x-minUniqueItems`).
fn check_n<S: VldSchema + JsonSchema>(name: &str, schema: S, patch: serde_json::Value) {
    let mut js = schema.json_schema();
    if let (Some(o), Some(p)) = (js.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            o.insert(k.clone(), v.clone());
        }
    }
    for _ in 0..300 {
        let v = vld_fake::fake_value(&js);
        if let Err(e) = schema.parse_value(&v) {
            panic!("{name}: {v} -> {e} (schema {js})");
        }
    }
}

fn none() -> serde_json::Value {
    serde_json::json!({})
}

#[test]
fn intersections_merge_constraints() {
    check_n(
        "u8_range",
        vld::intersection(vld::u8().min(10), vld::u8().max(12)),
        none(),
    );
    check_n(
        "multiple_of_lcm",
        vld::number().multiple_of(3.).multiple_of(5.),
        none(),
    );
    check_n(
        "int_multiple_of_lcm",
        vld::intersection(
            vld::number().int().multiple_of(4).min(1),
            vld::number().int().multiple_of(6).max(100),
        ),
        none(),
    );
    check_n(
        "string_lengths",
        vld::intersection(vld::string().min(5), vld::string().max(7)),
        none(),
    );
    check_n(
        "string_patterns",
        vld::intersection(
            vld::string().starts_with("ab"),
            vld::string().ends_with("yz"),
        ),
        none(),
    );
    check_n(
        "two_formats",
        vld::string().alphanumeric().uppercase(),
        none(),
    );
    check_n(
        "format_intersection",
        vld::intersection(vld::string().email(), vld::string().max(14)),
        none(),
    );
    check_n(
        "objects",
        vld::intersection(
            vld::object().field("a", vld::u8().min(5)),
            vld::object()
                .field("a", vld::u8().max(6))
                .field("b", vld::boolean()),
        ),
        none(),
    );
}

#[test]
fn formats_with_literals_and_lengths() {
    check_n(
        "email_prefix",
        vld::string().email().starts_with("ab"),
        none(),
    );
    check_n("uuid_suffix", vld::string().uuid().ends_with("ff"), none());
    check_n(
        "uuid_contains",
        vld::string().uuid().contains("abc"),
        none(),
    );
    check_n("url_max", vld::string().url().max(15), none());
    check_n("url_min", vld::string().url().min(60), none());
    check_n("ipv4_max", vld::string().ipv4().max(9), none());
    check_n("slug_min", vld::string().slug().min(40), none());
    check_n(
        "hostname_len",
        vld::string().hostname().min(70).max(80),
        none(),
    );
    check_n("nanoid_len", vld::string().nanoid().len(5), none());
    check_n("semver_min", vld::string().semver().min(12), none());
    check_n(
        "hex_prefix",
        vld::string().hex().starts_with("ff").max(6),
        none(),
    );
    check_n("emoji_len", vld::string().emoji().max(2), none());
    check_n("json_len", vld::string().json().min(30), none());
    check_n("base64_len", vld::string().base64().len(8), none());
    check_n(
        "date_prefix",
        vld::string().iso_date().starts_with("2021"),
        none(),
    );
}

#[test]
fn dates_with_bounds() {
    check_n(
        "date_range",
        vld::chrono::date().min("2020-03-01").max("2020-03-05"),
        none(),
    );
    check_n(
        "datetime_range",
        vld::chrono::datetime()
            .min("2021-06-01T10:00:00Z")
            .max("2021-06-01T10:00:30Z"),
        none(),
    );
    check_n(
        "time_range",
        vld::chrono::time().min("09:00:00").max("09:00:10"),
        none(),
    );
    let past = || serde_json::json!({"x-past": true});
    let future = || serde_json::json!({"x-future": true});
    check_n("date_past", vld::chrono::date().past(), past());
    check_n("date_future", vld::chrono::date().future(), future());
    check_n("datetime_past", vld::chrono::datetime().past(), past());
    check_n(
        "datetime_future",
        vld::chrono::datetime().future(),
        future(),
    );
    check_n(
        "date_future_max",
        vld::chrono::date().future().max("2999-01-01"),
        future(),
    );
}

#[test]
fn arrays_sets_maps_tuples() {
    check_n(
        "contains",
        vld::array(vld::u8().max(5))
            .contains(3)
            .min_contains(2)
            .max_contains(3),
        none(),
    );
    check_n(
        "contains_default",
        vld::array(vld::boolean()).contains(true).max_len(1),
        none(),
    );
    check_n(
        "contains_max_one",
        vld::array(vld::boolean())
            .contains(true)
            .max_contains(1)
            .min_len(3)
            .max_len(3),
        none(),
    );
    check_n(
        "set_bool_min",
        vld::set(vld::boolean()).min_size(2),
        serde_json::json!({"x-minUniqueItems": 2}),
    );
    check_n(
        "set_small_max",
        vld::set(vld::u8().max(2)).max_size(2),
        none(),
    );
    check_n(
        "set_range",
        vld::set(vld::u8().max(9)).min_size(3).max_size(5),
        serde_json::json!({"x-minUniqueItems": 3}),
    );
    check_n(
        "unique",
        vld::array(vld::u8().max(4)).unique().min_len(3),
        none(),
    );
    check_n("map", vld::map(vld::u8().max(3), vld::boolean()), none());
    check_n(
        "tuple",
        (vld::string().email(), vld::u8().min(3), vld::boolean()),
        none(),
    );
}
