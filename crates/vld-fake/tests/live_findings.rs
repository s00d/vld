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
        vld::string().regex(re("^[a-z]{3}[0-9]{2}$")).starts_with("ab"),
    );
}
