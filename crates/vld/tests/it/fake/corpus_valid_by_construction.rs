//! Corpus: a table of vld schemas whose generators are easy to get wrong
//! (every string format, formats with lengths and literals, one-sided and
//! tiny numeric ranges, `multipleOf`, intersections, dates with bounds,
//! `contains`, sets, maps, tuples, records, the 0.5 core keywords) — 300
//! seeded fake values each must pass the schema they came from.

use super::common::fake_value;
use serde_json::{Value, json};
use vld::json_schema::JsonSchema;
use vld::schema::VldSchema;

const DRAWS: usize = 300;

/// `DRAWS` fakes from `schema`'s JSON Schema with `patch` merged in (keywords
/// the 0.5 core emits, e.g. `x-past`) must all parse; the failure, if any.
fn failure<S: VldSchema + JsonSchema>(name: &str, schema: S, patch: Value) -> Option<String> {
    let mut js = schema.json_schema();
    if let (Some(o), Some(p)) = (js.as_object_mut(), patch.as_object()) {
        o.extend(p.clone());
    }
    (0..DRAWS).find_map(|_| {
        let v = fake_value(&js);
        schema
            .parse_value(&v)
            .err()
            .map(|e| format!("{name}: {v} -> {e} (schema {js})"))
    })
}

macro_rules! table {
    ($($name:literal => $schema:expr $(, $patch:expr)?;)*) => {{
        let mut failures: Vec<String> = Vec::new();
        $(
            let patch: Option<Value> = None $(.or(Some($patch)))?;
            failures.extend(failure($name, $schema, patch.unwrap_or_else(|| json!({}))));
        )*
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }};
}

/// Given every core string format, alone and with lengths or literals, when
/// fakes are generated, then each passes its schema.
#[test]
fn string_formats() {
    let s = vld::string;
    table! {
        "email" => s().email(); "url" => s().url(); "url_strict" => s().url_strict();
        "uri" => s().uri(); "uuid" => s().uuid(); "uuid_v1" => s().uuid_v1();
        "uuid_v4" => s().uuid_v4(); "uuid_v7" => s().uuid_v7(); "ip" => s().ip();
        "ipv4" => s().ipv4(); "ipv6" => s().ipv6(); "cidr" => s().cidr(); "mac" => s().mac();
        "hex" => s().hex(); "slug" => s().slug(); "color" => s().color();
        "currency_code" => s().currency_code(); "country_code" => s().country_code();
        "locale" => s().locale(); "cron" => s().cron(); "credit_card" => s().credit_card();
        "phone" => s().phone(); "phone_e164_strict" => s().phone_e164_strict();
        "semver" => s().semver(); "semver_full" => s().semver_full(); "jwt" => s().jwt();
        "ascii" => s().ascii(); "alpha" => s().alpha(); "alphanumeric" => s().alphanumeric();
        "lowercase" => s().lowercase(); "uppercase" => s().uppercase();
        "iso_date" => s().iso_date(); "iso_datetime" => s().iso_datetime();
        "iso_time" => s().iso_time(); "hostname" => s().hostname(); "cuid2" => s().cuid2();
        "ulid" => s().ulid(); "nanoid" => s().nanoid(); "emoji" => s().emoji();
        "json" => s().json(); "base64url" => s().base64url();
        "country" => s().country(); "country_alpha3" => s().country_alpha3();
        "iban" => s().iban(); "bic" => s().bic();
        "isbn" => s().isbn(); "ean" => s().ean(); "vin" => s().vin();
        "timezone" => s().timezone(); "mime" => s().mime(); "object_id" => s().object_id();
        "regex_syntax" => s().regex_syntax();
        "password" => s().min(12).require_lowercase().require_uppercase().require_digit()
            .require_symbol().max_repeat(1);
        "password_short" => s().max(6).require_symbol().require_digit();
        "password_long" => s().min(40).max(41).require_uppercase();
        "password_ascii" => s().ascii().require_lowercase().min(10);
        // lengths
        "base64url_len" => s().base64url().len(8); "email_max" => s().email().max(20);
        "hex_40" => s().hex().min(40).max(40); "alpha_1_3" => s().alpha().min(1).max(3);
        "alphanumeric_50" => s().alphanumeric().min(50); "lowercase_2" => s().lowercase().max(2);
        "uppercase_30" => s().uppercase().min(30).max(31); "email_min" => s().email().min(40);
        "ascii_3_4" => s().ascii().min(3).max(4); "url_max" => s().url().max(15);
        "url_min" => s().url().min(60); "ipv4_max" => s().ipv4().max(9);
        "slug_min" => s().slug().min(40); "hostname_len" => s().hostname().min(70).max(80);
        "nanoid_len" => s().nanoid().len(5); "semver_min" => s().semver().min(12);
        "emoji_len" => s().emoji().max(2); "json_len" => s().json().min(30);
        "base64_len" => s().base64().len(8);
        // literals
        "starts_ends" => s().starts_with("pre_").ends_with("_suf").min(10);
        "contains_max" => s().contains("@@").max(6);
        "regex_and_prefix" => s().pattern("^[a-z]{3}[0-9]{2}$").starts_with("ab");
        "email_prefix" => s().email().starts_with("ab"); "uuid_suffix" => s().uuid().ends_with("ff");
        "uuid_contains" => s().uuid().contains("abc");
        "hex_prefix" => s().hex().starts_with("ff").max(6);
        "date_prefix" => s().iso_date().starts_with("2021");
        "two_formats" => s().alphanumeric().uppercase();
    }
}

/// Given numbers with one-sided, exclusive, tiny and extreme ranges and
/// `multipleOf`, when fakes are generated, then each is in range.
#[test]
fn numbers() {
    let n = || vld::number().int();
    table! {
        "negative" => vld::number().negative(); "negative_int" => n().negative();
        "u64_top" => vld::u64().min(u64::MAX - 5);
        "i64_bottom" => vld::i64().min(i64::MIN).max(i64::MIN + 3);
        // default max 1000 used to swap with min
        "min_only" => n().min(5000); "max_only" => n().max(-10); "gt_only" => n().gt(7);
        "multiple_min" => n().multiple_of(7).min(5000); "multiple_max" => n().multiple_of(5).max(-12);
        "float_quarter" => vld::number().multiple_of(0.25).min(1.0).max(3.0);
        "float_half" => vld::number().multiple_of(0.5);
        "float_three" => vld::number().multiple_of(3.0).min(10.0).max(20.0);
        // narrower than the 2-decimal rounding step
        "tiny" => vld::number().min(0.001).max(0.002); "tiny_open" => vld::number().gt(0.0).lt(0.001);
        "neg_max" => vld::number().max(-5.5); "big_min" => vld::number().min(10_000.0);
        "decimal" => vld::decimal();
    }
}

/// Given intersections, when fakes are generated, then both sides' constraints
/// are merged (ranges, `multipleOf` LCM, lengths, literals, formats, fields).
#[test]
fn intersections_merge_constraints() {
    table! {
        "u8_range" => vld::intersection(vld::u8().min(10), vld::u8().max(12));
        "multiple_of_lcm" => vld::number().multiple_of(3.).multiple_of(5.);
        "int_multiple_of_lcm" => vld::intersection(
            vld::number().int().multiple_of(4).min(1),
            vld::number().int().multiple_of(6).max(100),
        );
        "string_lengths" => vld::intersection(vld::string().min(5), vld::string().max(7));
        "string_patterns" => vld::intersection(vld::string().starts_with("ab"), vld::string().ends_with("yz"));
        "format_intersection" => vld::intersection(vld::string().email(), vld::string().max(14));
        "objects" => vld::intersection(
            vld::object().field("a", vld::u8().min(5)),
            vld::object().field("a", vld::u8().max(6)).field("b", vld::boolean()),
        );
    }
}

/// Given dates, date-times and times with bounds, `x-past` / `x-future`, when
/// fakes are generated, then each lies inside its bounds.
#[test]
fn dates_with_bounds() {
    let past = || json!({"x-past": true});
    let future = || json!({"x-future": true});
    table! {
        "date_range" => vld::chrono::date().min("2020-03-01").max("2020-03-05");
        "datetime_range" => vld::chrono::datetime().min("2021-06-01T10:00:00Z").max("2021-06-01T10:00:30Z");
        "time_range" => vld::chrono::time().min("09:00:00").max("09:00:10");
        "date_past" => vld::chrono::date().past(), past();
        "date_future" => vld::chrono::date().future(), future();
        "datetime_past" => vld::chrono::datetime().past(), past();
        "datetime_future" => vld::chrono::datetime().future(), future();
        "date_future_max" => vld::chrono::date().future().max("2999-01-01"), future();
        "duration" => vld::duration();
    }
}

/// Given arrays with `contains`, sets, unique arrays, maps, tuples, records,
/// bytes and network types, when fakes are generated, then each passes.
#[test]
fn collections_and_other_types() {
    table! {
        "contains" => vld::array(vld::u8().max(5)).contains(3).min_contains(2).max_contains(3);
        "contains_default" => vld::array(vld::boolean()).contains(true).max_len(1);
        "contains_max_one" => vld::array(vld::boolean()).contains(true).max_contains(1).min_len(3).max_len(3);
        "set_bool_min" => vld::set(vld::boolean()).min_size(2), json!({"x-minUniqueItems": 2});
        "set_small_max" => vld::set(vld::u8().max(2)).max_size(2);
        "set_range" => vld::set(vld::u8().max(9)).min_size(3).max_size(5), json!({"x-minUniqueItems": 3});
        "unique" => vld::array(vld::u8().max(4)).unique().min_len(3);
        "map" => vld::map(vld::u8().max(3), vld::boolean());
        "tuple" => (vld::string().email(), vld::u8().min(3), vld::boolean());
        "record_min_keys" => vld::record(vld::u8().max(3)).min_keys(1).max_keys(4);
        "record_with_keys" => vld::record_with_keys(vld::string().pattern("^k[0-9]$"), vld::number().int());
        "bytes_base64" => vld::bytes().base64().min_len(4).max_len(6);
        "bytes_base64url" => vld::bytes().base64url(); "bytes_hex" => vld::bytes().hex().len(3);
        "ip_network" => vld::ip_network(); "socket_addr" => vld::socket_addr(); "path" => vld::path();
    }
}

/// Given the JSON Schemas the 0.5 core emits for offset-free times, naive and
/// naive-allowed date-times and bounded `anyOf` dates, when fakes are
/// generated, then they are real times / date-times inside the bounds, past
/// when `x-past` says so, and ULIDs fit 128 bits.
#[test]
fn core_shapes_are_generated_validly() {
    let time = json!({
        "type": "string", "format": "partial-time",
        "pattern": r"^([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$"
    });
    let naive = json!({
        "type": "string",
        "anyOf": [
            {"format": "date-time"},
            {"format": "naive-date-time",
             "pattern": r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])[Tt]([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$"}
        ]
    });
    let past = json!({
        "type": "string",
        "anyOf": [
            {"format": "date-time"},
            {"pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}$"}
        ],
        "x-past": true
    });
    let bounded = json!({
        "type": "string",
        "anyOf": [{"format": "date"}],
        "formatMinimum": "2020-03-01",
        "formatMaximum": "2020-03-02"
    });
    let ulid = json!({"type": "string", "format": "ulid"});
    let t = vld::string().iso_time();
    let dt = vld::string().iso_datetime();
    let past_dt = vld::chrono::datetime().past();
    let d = vld::chrono::date().min("2020-03-01").max("2020-03-02");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for _ in 0..DRAWS {
        let v = fake_value(&time);
        assert!(t.parse_value(&v).is_ok(), "time {v}");
        let v = fake_value(&naive);
        assert!(dt.parse_value(&v).is_ok(), "datetime {v}");
        let v = fake_value(&past);
        let when = past_dt
            .parse_value(&v)
            .unwrap_or_else(|e| panic!("{v}: {e}"));
        assert!(when.timestamp() < now, "{v} is not in the past");
        let v = fake_value(&bounded);
        let day = d.parse_value(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert!(
            ["2020-03-01", "2020-03-02"].contains(&day.to_string().as_str()),
            "{v}"
        );
        let v = fake_value(&ulid);
        let first = v.as_str().unwrap().as_bytes()[0];
        assert!((b'0'..=b'7').contains(&first), "{v}");
    }
}

/// Given decimals with scale / precision, money in every kind of currency, the number presets,
/// ranges over numbers and dates, and Unix-timestamp datetimes (also past / future), when fakes
/// are generated, then each passes: amounts fit the currency's minor units, range ends are
/// ordered, timestamps lie in range and on the right side of now.
#[test]
fn units_ranges_and_timestamps() {
    table! {
        "decimal_scale0" => vld::decimal().max_scale(0);
        "decimal_scale3_exact" => vld::decimal().scale(3);
        "decimal_precision" => vld::decimal().max_precision(4).max_scale(2);
        "decimal_precision_exact" => vld::decimal().scale(2).max_precision(3);
        "decimal_exclusive" => vld::decimal().gt("0").lt("0.05").max_scale(2);
        "decimal_negative" => vld::decimal().negative().max_scale(1);
        "money" => vld::money();
        "money_allow_list" => vld::money().currencies(&["JPY", "BHD", "CLF"]);
        "money_positive" => vld::money().amount(vld::decimal().positive());
        "percent" => vld::percent();
        "ratio" => vld::ratio();
        "range_int" => vld::range(vld::i32());
        "range_number" => vld::range(vld::number().min(-5.0).max(5.0));
        "range_date" => vld::range(vld::chrono::date()).keys("from", "to");
        "range_datetime" => vld::range(vld::chrono::datetime().with_timezone_only());
        "range_unix" => vld::range(vld::chrono::datetime().unix_millis());
        "unix_seconds" => vld::chrono::datetime().unix_seconds();
        "unix_millis_past" => vld::chrono::datetime().unix_millis().past();
        "unix_future" => vld::chrono::datetime().unix_seconds().future();
        "unix_bounded" => vld::chrono::datetime().unix_seconds().min("2020-01-01T00:00:00Z").max("2020-01-02T00:00:00Z");
    }
}

/// Given every geo schema (coordinates, points, each GeoJSON type, features), when fakes are
/// generated, then each passes: rings are closed, pairs are `[lng, lat]` in range.
#[cfg(feature = "geo")]
#[test]
fn geo_shapes() {
    table! {
        "latitude" => vld::latitude();
        "longitude" => vld::longitude();
        "geo_point" => vld::geo_point();
        "geojson" => vld::geojson();
        "polygons" => vld::geojson().types(&["Polygon", "MultiPolygon"]);
        "lines" => vld::geojson().types(&["LineString", "MultiLineString"]);
        "features" => vld::geojson().types(&["Feature", "FeatureCollection"]);
    }
}
