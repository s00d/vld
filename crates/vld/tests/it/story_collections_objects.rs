//! Stories: objects (unknown-key modes, key counts, `pick` / `omit` / `extend` / `merge`,
//! `partial` / `required` / `field_optional`, `when` rules, `catchall`), collections (array,
//! set, map, record, tuple), recursion with `lazy`, pipelines (`transform`, `pipe`,
//! `preprocess`), custom checks (`refine`, `super_refine`, `custom`) and combinators (unions,
//! `union!`, discriminated unions, intersections). Plain semantics of random trees of these are
//! also the model in `model_schema.rs`; these stories pin the edge cases and accessors.

use crate::common::*;

use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use vld::error::{IssueCode, SizeOrigin, StringValidation};
use vld::prelude::*;

fn person() -> vld::object::ZObject {
    vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int().min(0))
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

fn sorted(mut v: Vec<(String, String)>) -> Vec<(String, String)> {
    v.sort();
    v
}

/// Given a sign-up object of `name` (non-empty) and `age` (integer ≥ 0)
/// When clients send valid, broken, empty, `null` and over-full bodies, in every unknown-key mode
/// Then the valid body comes back, a non-object is `invalid_type`, each bad field is reported at
/// its path, an absent key is `missing_field` and a present `null` is `invalid_type`; unknown keys
/// are stripped by default (and after `passthrough().strip()`), kept unvalidated under
/// `passthrough()`, each rejected as `unrecognized_field` under `strict()`; `min_keys` /
/// `max_keys` count input keys; `keyof` lists the fields in order.
#[test]
fn signup_object_in_every_unknown_key_mode() {
    let out = person()
        .parse_value(&json!({"name": "Alex", "age": 25}))
        .unwrap();
    assert_eq!(out["name"], json!("Alex"));
    assert_eq!(out["age"], json!(25));
    assert_eq!(
        person().parse_value(&json!([])).unwrap_err().issues[0].code,
        IssueCode::InvalidType {
            expected: "object".into(),
            received: "array".into()
        }
    );

    let err = person()
        .parse_value(&json!({"name": "", "age": -1}))
        .unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[(".name", "too_small"), (".age", "too_small")])
    );
    let err = person().parse_value(&json!({})).unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[(".name", "missing_field"), (".age", "missing_field")])
    );
    let err = person()
        .parse_value(&json!({"name": null, "age": null}))
        .unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[(".name", "invalid_type"), (".age", "invalid_type")])
    );

    let extra = json!({"name": "Alex", "age": 1, "extra": [1, "x"]});
    for schema in [person(), person().passthrough().strip()] {
        let out = schema.parse_value(&extra).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.get("extra").is_none());
    }
    let out = person().passthrough().parse_value(&extra).unwrap();
    assert_eq!(out["extra"], json!([1, "x"]));

    let err = person()
        .strict()
        .parse_value(&json!({"name": "Alex", "age": 1, "x": 1, "é": 2}))
        .unwrap_err();
    assert_eq!(
        sorted(issues(&err)),
        pairs(&[(".x", "unrecognized_field"), (".é", "unrecognized_field")])
    );

    let counted = vld::object()
        .field("name", vld::string())
        .min_keys(1)
        .max_keys(2)
        .passthrough();
    assert!(counted.parse_value(&json!({"name": "Alex"})).is_ok());
    assert!(matches!(
        counted.parse_value(&json!({})).unwrap_err().issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Object,
            ..
        }
    ));
    let err = counted
        .parse_value(&json!({"name": "A", "b": 1, "c": 2}))
        .unwrap_err();
    assert!(matches!(
        err.issues[0].code,
        IssueCode::TooBig {
            origin: SizeOrigin::Object,
            ..
        }
    ));

    assert_eq!(person().keyof(), ["name", "age"]);
    assert!(vld::object().keyof().is_empty());
}

/// Given an object `{a: string, b: number, c: boolean}`
/// When it is reshaped with `pick`, `omit`, `extend` and `merge`
/// Then a dropped key becomes unknown (stripped, rejected under `strict()`, not validated), `pick`
/// keeps declaration order and ignores unknown names, `omit` of an absent name is a no-op; the
/// later schema wins on `merge`, `extend` keeps the other schema's `strict`, `min_keys` and
/// `when` rules (whose target is a known key), and a repeated `field()` replaces the first.
#[test]
fn reshaping_objects() {
    let abc = || {
        vld::object()
            .field("a", vld::string())
            .field("b", vld::number())
            .field("c", vld::boolean())
    };
    let input = json!({"a": "x", "b": "not a number", "c": true});

    let out = abc().pick(&["a", "c"]).parse_value(&input).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.get("b").is_none());
    let err = abc()
        .strict()
        .pick(&["a", "c"])
        .parse_value(&input)
        .unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".b", "unrecognized_field")]));
    assert_eq!(abc().pick(&["c", "a", "zzz"]).keyof(), ["a", "c"]);

    let out = abc().omit("b").parse_value(&input).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.get("b").is_none());
    let err = abc().strict().omit("b").parse_value(&input).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".b", "unrecognized_field")]));
    assert_eq!(abc().omit("missing").keyof(), ["a", "b", "c"]);

    let extended = vld::object()
        .field("a", vld::string().min(1))
        .extend(vld::object().field("b", vld::number()));
    assert_eq!(
        extended
            .parse_value(&json!({"a": "x", "b": 42}))
            .unwrap()
            .len(),
        2
    );

    let merged = vld::object()
        .field("a", vld::string().min(1))
        .merge(vld::object().field("a", vld::string().min(10)));
    assert_eq!(merged.keyof(), ["a"]);
    let err = merged.parse_value(&json!({"a": "hi"})).unwrap_err();
    assert!(matches!(
        &err.issues[0].code,
        IssueCode::TooSmall { minimum, .. } if minimum == &json!(10)
    ));
    assert_eq!(path_of(&err.issues[0]), ".a");

    let other = vld::object()
        .field("b", vld::string())
        .strict()
        .min_keys(2)
        .when("b", "x", "c", vld::string().min(3));
    let m = vld::object().field("a", vld::string()).extend(other);
    let err = m.parse(r#"{"a": "1", "b": "y", "z": 1}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".z", "unrecognized_field")]));
    assert!(m.parse(r#"{"a": "1"}"#).is_err());
    let err = m.parse(r#"{"a": "1", "b": "x", "c": "ab"}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".c", "too_small")]));
    assert!(m.parse(r#"{"a": "1", "b": "x", "c": "abc"}"#).is_ok());

    let o = vld::object()
        .field("a", vld::string())
        .field("a", vld::number());
    assert_eq!(o.keyof(), vec!["a".to_string()]);
    assert!(o.parse(r#"{"a": 1}"#).is_ok());
    #[cfg(feature = "openapi")]
    assert_eq!(
        vld::openapi::JsonSchema::json_schema(&o)["required"],
        json!(["a"])
    );
}

/// Given a PATCH body built with `partial()`, `required()` and `field_optional`
/// When fields are absent, `null` or present
/// Then `partial` accepts absent and `null` (output `null`) but validates present values, keeps an
/// inner `with_default` / `catch`, and is shallow (a nested object keeps its required keys);
/// `required()` undoes it (absent or `null` is `missing_field`); `field_optional` maps absent and
/// `null` to `null`, validates present values and keeps an inner default.
#[test]
fn patch_body_with_optional_fields() {
    let s = person().partial();
    for input in [json!({}), json!({"name": null, "age": null})] {
        let out = s.parse_value(&input).unwrap();
        assert_eq!(out["name"], json!(null), "{input}");
        assert_eq!(out["age"], json!(null), "{input}");
    }
    let err = s.parse_value(&json!({"name": "", "age": "x"})).unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[(".name", "too_small"), (".age", "invalid_type")])
    );

    let kept = vld::object()
        .field("n", vld::number().with_default(5.0))
        .field("s", vld::string().min(3).catch("anon".to_string()))
        .field("plain", vld::string())
        .partial();
    assert_eq!(
        serde_json::Value::Object(kept.parse_value(&json!({})).unwrap()),
        json!({"n": 5.0, "s": "anon", "plain": null})
    );

    let shallow = vld::object()
        .field("name", vld::string().min(1))
        .field("addr", vld::object().field("city", vld::string()))
        .partial();
    assert!(shallow.parse_value(&json!({})).is_ok());
    let err = shallow.parse_value(&json!({"addr": {}})).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".addr.city", "missing_field")]));

    let req = person().partial().required();
    for input in [json!({"age": 1}), json!({"name": null, "age": 1})] {
        let err = req.parse_value(&input).unwrap_err();
        assert_eq!(
            issues(&err),
            pairs(&[(".name", "missing_field")]),
            "{input}"
        );
    }
    assert!(req.parse_value(&json!({"name": "a", "age": 1})).is_ok());

    let fo = vld::object()
        .field("name", vld::string())
        .field_optional("age", vld::number().int().min(0));
    for input in [json!({"name": "Bob"}), json!({"name": "Bob", "age": null})] {
        assert_eq!(
            fo.parse_value(&input).unwrap()["age"],
            json!(null),
            "{input}"
        );
    }
    assert_eq!(
        fo.parse_value(&json!({"name": "Bob", "age": 30})).unwrap()["age"],
        json!(30)
    );
    let err = fo
        .parse_value(&json!({"name": "Bob", "age": -5}))
        .unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".age", "too_small")]));

    let def = vld::object().field_optional("n", vld::number().with_default(1.0));
    assert_eq!(def.parse_value(&json!({})).unwrap()["n"], json!(1.0));
    assert_eq!(def.parse_value(&json!({"n": 2})).unwrap()["n"], json!(2.0));
}

/// Given objects with conditional rules (`when(field, value, target, schema)`) and a `catchall`
/// When the condition matches or not, and unknown keys arrive
/// Then the extra schema applies only on a match (string or numeric condition), a missing target
/// is validated as `null`; a catchall validates every unknown key (each bad one at its path) and
/// overrides `strict()`.
#[test]
fn conditional_rules_and_catchall() {
    let s = vld::object()
        .field("role", vld::string())
        .field_optional("admin_key", vld::string())
        .when("role", "admin", "admin_key", vld::string().min(10));
    assert!(
        s.parse(r#"{"role": "admin", "admin_key": "super-secret-key"}"#)
            .is_ok()
    );
    assert!(s.parse(r#"{"role": "user"}"#).is_ok());
    assert!(s.parse(r#"{"role": "user", "admin_key": "x"}"#).is_ok());
    let err = s
        .parse(r#"{"role": "admin", "admin_key": "short"}"#)
        .unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".admin_key", "too_small")]));
    let err = s.parse(r#"{"role": "admin"}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".admin_key", "invalid_type")]));

    let n = vld::object()
        .field("type", vld::number().int())
        .field_optional("detail", vld::string())
        .when("type", json!(1), "detail", vld::string().min(5));
    let err = n.parse(r#"{"type": 1, "detail": "ab"}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".detail", "too_small")]));
    assert!(n.parse(r#"{"type": 2, "detail": "ab"}"#).is_ok());

    let c = vld::object()
        .field("name", vld::string())
        .catchall(vld::number().int());
    assert_eq!(
        c.parse(r#"{"name":"Alex","score":42}"#).unwrap()["score"],
        json!(42)
    );
    let err = c
        .parse_value(&json!({"name": "Alex", "score": "nope", "rank": 1.5}))
        .unwrap_err();
    assert_eq!(
        sorted(issues(&err)),
        pairs(&[(".rank", "not_int"), (".score", "invalid_type")])
    );

    let strict = vld::object()
        .field("a", vld::string())
        .strict()
        .catchall(vld::string());
    assert_eq!(
        strict.parse(r#"{"a":"x","b":"y"}"#).unwrap()["b"],
        json!("y")
    );
    let err = strict.parse(r#"{"a":"x","b":1}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".b", "invalid_type")]));
}

/// Given tag lists: arrays with length bounds, exact length, `non_empty`, `contains` counts and
/// `unique`
/// When lists of every size and with bad elements are parsed
/// Then bounds report `too_small` / `too_big` / `invalid_length` with the array origin and bound,
/// every bad element is reported at its index (nested indexes too); `contains` with
/// `min_contains` / `max_contains` counts matches (`missing_contains` without counts,
/// `min_contains(0)` makes it optional); `unique` flags each repeat, comparing numbers by value
/// and objects regardless of key order.
#[test]
fn tag_lists() {
    let a = vld::array(vld::string()).min_len(1).max_len(3);
    assert_eq!(
        a.parse_value(&json!(["a", "b"])).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(
        a.parse_value(&json!([])).unwrap_err().issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Array,
            minimum: json!(1),
            inclusive: true
        }
    );
    assert_eq!(
        a.parse_value(&json!(["a", "b", "c", "d"]))
            .unwrap_err()
            .issues[0]
            .code,
        IssueCode::TooBig {
            origin: SizeOrigin::Array,
            maximum: json!(3),
            inclusive: true
        }
    );
    assert_eq!(
        a.parse_value(&json!("a")).unwrap_err().issues[0].code,
        IssueCode::InvalidType {
            expected: "array".into(),
            received: "string".into()
        }
    );
    let exact = vld::array(vld::any()).len(2);
    assert!(exact.parse_value(&json!([1, 2])).is_ok());
    assert_eq!(
        exact.parse_value(&json!([1])).unwrap_err().issues[0].code,
        IssueCode::InvalidLength {
            origin: SizeOrigin::Array,
            expected: 2
        }
    );
    let non_empty = vld::array(vld::any()).non_empty();
    assert_eq!(
        issues(&non_empty.parse_value(&json!([])).unwrap_err()),
        pairs(&[("", "too_small")])
    );

    let err = vld::array(vld::string().min(2))
        .parse_value(&json!(["ok", "x", "fine", "y", 3]))
        .unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[
            ("[1]", "too_small"),
            ("[3]", "too_small"),
            ("[4]", "invalid_type")
        ])
    );
    let err = vld::array(vld::array(vld::number().int()))
        .parse_value(&json!([[1], [2, "x"]]))
        .unwrap_err();
    assert_eq!(issues(&err), pairs(&[("[1][1]", "invalid_type")]));

    let counted = vld::array(vld::number().int())
        .contains(json!(2))
        .min_contains(2)
        .max_contains(3);
    assert!(counted.parse_value(&json!([2, 1, 2])).is_ok());
    assert!(counted.parse_value(&json!([2, 2, 2])).is_ok());
    let contains_code =
        |v: serde_json::Value| counted.parse_value(&v).unwrap_err().issues[0].code.clone();
    for few in [json!([1, 3]), json!([1, 2])] {
        assert!(matches!(
            contains_code(few),
            IssueCode::TooSmall {
                origin: SizeOrigin::Contains,
                ..
            }
        ));
    }
    assert!(matches!(
        contains_code(json!([2, 2, 2, 2])),
        IssueCode::TooBig {
            origin: SizeOrigin::Contains,
            ..
        }
    ));
    assert_eq!(
        vld::array(vld::any())
            .contains(json!(2))
            .parse_value(&json!([1, 3]))
            .unwrap_err()
            .issues[0]
            .code,
        IssueCode::MissingContains
    );
    assert!(
        vld::array(vld::any())
            .contains(1)
            .min_contains(0)
            .parse("[]")
            .is_ok()
    );

    let u = vld::array(vld::any()).unique();
    assert!(u.parse_value(&json!([1, 2, "1"])).is_ok());
    let err = u.parse("[1, 2, 1.0, 2, 3]").unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[("[2]", "not_unique"), ("[3]", "not_unique")])
    );
    assert!(u.parse(r#"[{"a":1,"b":2},{"b":2,"a":1.0}]"#).is_err());
    // Numbers compare by exact value, also past 2^53 and at the edges of i64 / u64.
    for distinct in [
        "[2, 1.5]",
        "[-9007199254740993, -9007199254740992]",
        "[18446744073709551615, 18446744073709551614]",
    ] {
        assert!(u.parse(distinct).is_ok(), "{distinct}");
    }
    for same in [
        "[0, -0.0]",
        "[9.2e18, 9200000000000000000]",
        "[9.3e18, 9300000000000000000]",
        "[1.8446744073709552e19, 1.8446744073709552e19]",
    ] {
        assert_eq!(
            issues(&u.parse(same).unwrap_err()),
            pairs(&[("[1]", "not_unique")]),
            "{same}"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Admin,
    User,
}

/// Given keyed collections: `set`, `map` (from `[key, value]` pairs), `record` (string keys),
/// `record_with_keys` (parsed keys) and `ordered()` records
/// When entries are valid, bad, duplicated or out of the size bounds
/// Then sets deduplicate and count unique elements for `min_size` / `max_size`, element errors
/// carry their index; maps report a bad key or value at the entry, a non-pair entry as
/// `invalid_map_entry`, a repeated key as `duplicate_key` (with the key as received) and expose
/// their key / value schemas; records report each bad value at its key, count keys for
/// `min_keys` / `max_keys`, report a bad key and a bad value under the same key both, accept a
/// serde enum or `uuid` string as key; `ordered()` yields a `BTreeMap` sorted by parsed key; a
/// named component inside any collection or object field is collected for OpenAPI.
#[test]
fn keyed_collections() {
    let s = vld::set(vld::string());
    let out = s.parse(r#"["a", "b", "a"]"#).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.contains("a") && out.contains("b"));
    assert!(s.parse("[]").unwrap().is_empty());
    assert_eq!(
        issues(&s.parse_value(&json!("a")).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
    let sized = vld::set(vld::number().int()).min_size(2).max_size(2);
    assert!(sized.parse("[1, 2]").is_ok());
    assert!(sized.parse("[1, 2, 2, 1]").is_ok());
    assert_eq!(
        sized.parse("[1, 1]").unwrap_err().issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Set,
            minimum: json!(2),
            inclusive: true
        }
    );
    assert_eq!(
        sized.parse("[1, 2, 3]").unwrap_err().issues[0].code,
        IssueCode::TooBig {
            origin: SizeOrigin::Set,
            maximum: json!(2),
            inclusive: true
        }
    );
    let err = vld::set(vld::string().non_empty())
        .parse(r#"["a", "", "b", 1]"#)
        .unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[("[1]", "too_small"), ("[3]", "invalid_type")])
    );

    let m = vld::map(vld::string(), vld::number().int());
    let out = m.parse(r#"[["a", 1], ["b", 2]]"#).unwrap();
    assert_eq!((out.len(), out["a"], out["b"]), (2, 1, 2));
    assert!(m.parse("[]").unwrap().is_empty());
    assert_eq!(m.key_schema().parse_value(&json!("k")).unwrap(), "k");
    assert_eq!(m.value_schema().parse_value(&json!(3)).unwrap(), 3);
    let checked = vld::map(vld::string().min(3), vld::number().int().positive());
    let err = checked
        .parse_value(&json!([["abc", 1], ["ab", 1], ["xyz", -1], "bad", ["a"]]))
        .unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[
            ("[1]", "too_small"),
            ("[2]", "too_small"),
            ("[3]", "invalid_map_entry"),
            ("[4]", "invalid_map_entry")
        ])
    );
    assert_eq!(
        issues(&checked.parse_value(&json!({"a": 1})).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
    let err = vld::map(vld::string(), vld::number())
        .parse(r#"[["a", 1], ["a", 2]]"#)
        .unwrap_err();
    assert_eq!(issues(&err), pairs(&[("[1]", "duplicate_key")]));
    assert_eq!(err.issues[0].received, Some(json!("a")));

    let r = vld::record(vld::number().int().positive());
    let out = r.parse_value(&json!({"a": 1, "b": 2, "c": 3})).unwrap();
    assert_eq!((out.len(), out.get("b")), (3, Some(&2)));
    let err = r
        .parse_value(&json!({"a": 1, "b": -1, "é": "x"}))
        .unwrap_err();
    assert_eq!(
        sorted(issues(&err)),
        pairs(&[(".b", "too_small"), (".é", "invalid_type")])
    );
    assert_eq!(
        r.parse_value(&json!([1])).unwrap_err().issues[0].code,
        IssueCode::InvalidType {
            expected: "object".into(),
            received: "array".into()
        }
    );
    let rk = vld::record(vld::string()).min_keys(1).max_keys(2);
    assert!(rk.parse_value(&json!({"a": "1"})).is_ok());
    assert!(matches!(
        rk.parse_value(&json!({})).unwrap_err().issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Record,
            ..
        }
    ));
    assert!(matches!(
        rk.parse_value(&json!({"a": "1", "b": "2", "c": "3"}))
            .unwrap_err()
            .issues[0]
            .code,
        IssueCode::TooBig {
            origin: SizeOrigin::Record,
            ..
        }
    ));

    let keyed = vld::record_with_keys(vld::from_str::<u32>(), vld::string());
    let m: HashMap<u32, String> = keyed.parse(r#"{"1": "a", "20": "b"}"#).unwrap();
    assert_eq!(m[&20], "b");
    let err = keyed.parse(r#"{"x": "a", "2": 3}"#).unwrap_err();
    assert_eq!(
        sorted(issues(&err)),
        pairs(&[(".2", "invalid_type"), (".x", "invalid_format")])
    );
    assert_eq!(keyed.parse(r#"{"x": 1}"#).unwrap_err().issues.len(), 2);

    let uuid_keys = vld::record_with_keys(vld::string().uuid(), vld::boolean());
    assert!(
        uuid_keys
            .parse(r#"{"67e55044-10b1-426f-9247-bb680e5fe0c8": true}"#)
            .is_ok()
    );
    assert_eq!(
        uuid_keys.parse(r#"{"nope": true}"#).unwrap_err().issues[0].code,
        IssueCode::InvalidString {
            validation: StringValidation::Uuid
        }
    );
    let roles = vld::record_with_keys(vld::from_serde::<Role>(), vld::number().int())
        .min_keys(1)
        .max_keys(2);
    assert_eq!(
        roles.parse(r#"{"admin": 1, "user": 2}"#).unwrap()[&Role::User],
        2
    );
    assert_eq!(
        issues(&roles.parse("{}").unwrap_err()),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(&roles.parse("[]").unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
    let _ = Role::Admin;

    let m: BTreeMap<String, f64> = vld::record(vld::number())
        .ordered()
        .parse(r#"{"b": 2, "a": 1, "c": 3}"#)
        .unwrap();
    assert_eq!(
        m.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    let ord: ZOrderedRecord<ZBoolean, vld::collections::KeySchema<vld::primitives::ZFromStr<i32>>> =
        vld::record_with_keys(vld::from_str::<i32>(), vld::boolean())
            .max_keys(3)
            .ordered();
    let m = ord
        .parse(r#"{"10": true, "-1": false, "2": true}"#)
        .unwrap();
    assert_eq!(m.keys().copied().collect::<Vec<_>>(), [-1, 2, 10]);
    assert!(
        ord.parse(r#"{"1": true, "2": true, "3": true, "4": true}"#)
            .is_err()
    );
    let _: HashMap<String, bool> = vld::record(vld::boolean()).parse("{}").unwrap();

    // A named component behind any collection (or an object field made optional / required)
    // is registered for the OpenAPI document.
    #[cfg(feature = "openapi")]
    {
        use vld::openapi::CollectNestedSchemas;
        fn tag_schema() -> serde_json::Value {
            json!({"type": "string"})
        }
        let tag = || {
            vld::nested_named(
                "Tag",
                |v: &serde_json::Value| vld::string().parse_value(v),
                Some(tag_schema),
                None,
            )
        };
        fn names(s: &impl CollectNestedSchemas) -> Vec<&'static str> {
            let mut out = Vec::new();
            s.collect_nested_schemas(&mut out);
            out.into_iter().map(|(n, _)| n).collect()
        }
        assert_eq!(names(&vld::array(tag())), ["Tag"]);
        assert_eq!(names(&vld::set(tag())), ["Tag"]);
        assert_eq!(names(&vld::map(vld::string(), tag())), ["Tag"]);
        assert_eq!(names(&vld::record(tag())), ["Tag"]);
        assert_eq!(names(&vld::object().field_optional("t", tag())), ["Tag"]);
        assert_eq!(names(&vld::object().field("t", tag()).partial()), ["Tag"]);
        assert_eq!(
            names(&vld::object().field("t", tag()).partial().required()),
            ["Tag"]
        );
        let obj = vld::object().field("t", tag());
        assert_eq!(
            obj.to_json_schema(),
            vld::openapi::JsonSchema::json_schema(&obj)
        );
        assert_eq!(obj.to_json_schema()["required"], json!(["t"]));
    }
}

/// Given positional tuples of 2, 3, 7 and 12 schemas
/// When rows of the right and wrong lengths and types arrive
/// Then each position gets its type, a wrong length is `invalid_length` (tuple origin, expected
/// arity), a non-array is `invalid_type`, and each bad element is reported at its index.
#[test]
fn positional_rows() {
    let t = (vld::string(), vld::number().int());
    assert_eq!(
        t.parse_value(&json!(["hello", 42])).unwrap(),
        ("hello".to_string(), 42)
    );
    let t3 = (vld::string(), vld::number(), vld::boolean());
    assert_eq!(
        t3.parse_value(&json!(["hi", 3.5, true])).unwrap(),
        ("hi".to_string(), 3.5, true)
    );
    for bad in [json!(["hello"]), json!(["a", 1, 2])] {
        assert_eq!(
            t.parse_value(&bad).unwrap_err().issues[0].code,
            IssueCode::InvalidLength {
                origin: SizeOrigin::Tuple,
                expected: 2
            }
        );
    }
    let err = t.parse_value(&json!([42, "hello"])).unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[("[0]", "invalid_type"), ("[1]", "invalid_type")])
    );
    assert_eq!(
        issues(&t.parse_value(&json!({"0": "a"})).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );

    let n = || vld::number().int();
    let twelve = (
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        vld::string(),
    );
    let out = twelve.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,"z"]"#).unwrap();
    assert_eq!((out.10, out.11.as_str()), (11, "z"));
    assert_eq!(
        twelve.parse("[1,2,3]").unwrap_err().issues[0].code,
        IssueCode::InvalidLength {
            origin: SizeOrigin::Tuple,
            expected: 12
        }
    );
    let err = twelve.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,12]"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[("[11]", "invalid_type")]));
    let seven = (n(), n(), n(), n(), n(), n(), vld::boolean());
    assert!(seven.parse("[1,2,3,4,5,6,true]").unwrap().6);
}

fn tree() -> vld::object::ZObject {
    vld::object()
        .field("value", vld::number().int())
        .field("children", vld::array(vld::lazy(tree)))
}

/// Given a recursive tree schema built with `lazy`
/// When a deep tree, a tree with a bad leaf and repeated parses run
/// Then the tree parses, the bad leaf is reported at its full path, a lazy wrapper validates like
/// its schema, and the factory runs once (cached).
#[test]
fn recursive_tree() {
    let input = r#"{"value": 1, "children": [
        {"value": 2, "children": []},
        {"value": 3, "children": [{"value": 4, "children": []}]}
    ]}"#;
    assert_eq!(
        tree().parse(input).unwrap()["children"][1]["children"][0]["value"],
        json!(4)
    );
    let bad =
        r#"{"value": 1, "children": [{"value": 2, "children": [{"value": "x", "children": []}]}]}"#;
    assert_eq!(
        issues(&tree().parse(bad).unwrap_err()),
        pairs(&[(".children[0].children[0].value", "invalid_type")])
    );

    let s = vld::lazy(|| vld::string().min(3));
    assert_eq!(s.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(
        issues(&s.parse(r#""hi""#).unwrap_err()),
        pairs(&[("", "too_small")])
    );

    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let counted = vld::lazy(|| {
        CALLS.fetch_add(1, Ordering::Relaxed);
        vld::string()
    });
    let _ = counted.parse(r#""a""#);
    let _ = counted.parse(r#""b""#);
    assert_eq!(CALLS.load(Ordering::Relaxed), 1);
}

/// Given pipelines: `transform` (map the output), `pipe` (feed it to another schema) and
/// `preprocess` (rewrite the input first)
/// When good and bad inputs flow through
/// Then each stage sees the previous output; a first-stage error stops the pipeline with its own
/// code; `optional().transform` can fill a default; `pipe` exposes both stages.
#[test]
fn pipelines() {
    let len = vld::string().transform(|s| s.chars().count());
    assert_eq!(len.parse_value(&json!("héllo")).unwrap(), 5);
    assert_eq!(
        issues(&len.parse_value(&json!(1)).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
    let defaulted = vld::string()
        .optional()
        .transform(|opt| opt.unwrap_or_else(|| "default".to_string()));
    assert_eq!(defaulted.parse_value(&json!(null)).unwrap(), "default");
    assert_eq!(defaulted.parse_value(&json!("hello")).unwrap(), "hello");

    let piped = vld::string()
        .transform(|s| s.len())
        .pipe(vld::number().min(3.0));
    assert_eq!(piped.parse_value(&json!("hello")).unwrap(), 5.0);
    assert_eq!(
        issues(&piped.parse_value(&json!("hi")).unwrap_err()),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(&piped.parse_value(&json!(1)).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
    let two = vld::string().pipe(vld::string().min(3));
    assert_eq!(two.first_schema().parse_value(&json!("a")).unwrap(), "a");
    assert_eq!(
        issues(&two.second_schema().parse_value(&json!("a")).unwrap_err()),
        pairs(&[("", "too_small")])
    );

    let pre = vld::preprocess(
        |v| match v.as_str() {
            Some(s) => json!(s.trim()),
            None => v.clone(),
        },
        vld::string().min(1),
    );
    assert_eq!(pre.parse_value(&json!("  hello  ")).unwrap(), "hello");
    assert_eq!(
        issues(&pre.parse_value(&json!("   ")).unwrap_err()),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(&pre.parse_value(&json!(5)).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );
}

fn custom(code: &str) -> IssueCode {
    IssueCode::Custom { code: code.into() }
}

/// Given business rules: `refine` (one predicate), `super_refine` (push several issues, or build
/// one with a path) and `custom` (a whole parser closure)
/// When values pass and fail
/// Then `refine` runs only after the inner schema succeeds and reports one `custom` issue with
/// its message; `super_refine` reports every pushed issue in order (none: the value passes) and
/// an `IssueBuilder` issue carries its code, message and path; `custom` returns the closure's
/// value or a `custom` issue with the closure's message and the received value.
#[test]
fn business_rules() {
    let even = vld::number().int().refine(|n| n % 2 == 0, "Must be even");
    assert_eq!(even.parse_value(&json!(4)).unwrap(), 4);
    let err = even.parse_value(&json!(3)).unwrap_err();
    assert_eq!(issues(&err), pairs(&[("", "custom")]));
    assert_eq!(err.issues[0].code, custom("custom"));
    assert_eq!(err.issues[0].message, "Must be even");
    assert_eq!(
        issues(&even.parse_value(&json!("x")).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );

    let s = vld::string().super_refine(|s, errors| {
        if s.len() < 3 {
            errors.push(custom("too_short"), "Too short");
        }
        if !s.contains('@') {
            errors.push(custom("no_at"), "Missing @");
        }
    });
    let err = s.parse(r#""hi""#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[("", "too_short"), ("", "no_at")]));
    assert_eq!(err.issues[1].message, "Missing @");
    assert_eq!(s.parse(r#""a@bc""#).unwrap(), "a@bc");
    let noop = vld::number().int().super_refine(|_n, _errors| {});
    assert_eq!(noop.parse("42").unwrap(), 42);

    let built = vld::string().super_refine(|s, errors| {
        if !s.contains('@') {
            errors
                .issue(custom("no_at"))
                .message("Missing @ symbol")
                .path_field("contact")
                .finish();
        }
    });
    let err = built.parse(r#""ab""#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".contact", "no_at")]));
    assert_eq!(err.issues[0].message, "Missing @ symbol");

    let whole = vld::custom(|v: &serde_json::Value| {
        let n = v.as_i64().ok_or_else(|| "Expected integer".to_string())?;
        if n % 2 == 0 {
            Ok(n)
        } else {
            Err("Must be even".to_string())
        }
    });
    assert_eq!(whole.parse("4").unwrap(), 4);
    let err = whole.parse("5").unwrap_err();
    assert_eq!(err.issues[0].code, custom("custom"));
    assert_eq!(err.issues[0].message, "Must be even");
    assert_eq!(err.issues[0].received, Some(json!(5)));
    assert_eq!(
        whole.parse(r#""x""#).unwrap_err().issues[0].message,
        "Expected integer"
    );
}

/// Given unions (`union`, `.or()`, `union3`, `union!` with 7, 8 and 13 members, literal unions)
/// and intersections (`intersection`, `.and()`)
/// When values match one, none or both sides
/// Then a union returns the first matching side (`Either` left / right), exposes its members, and
/// on failure reports `invalid_union` with the received value followed by every member's issues
/// tagged `[variant N]` (nested `union!` members flattened, prefixes never stacked); an
/// intersection needs both sides and merges both sides' issues.
#[test]
fn unions_and_intersections() {
    let s = vld::union(vld::string(), vld::number().int());
    assert_eq!(
        s.parse_value(&json!("hello")).unwrap().left().unwrap(),
        "hello"
    );
    assert_eq!(s.parse_value(&json!(42)).unwrap().right().unwrap(), 42);
    let or = vld::string().or(vld::number().int());
    assert!(or.parse("\"x\"").unwrap().is_left());
    assert!(or.parse("1").unwrap().is_right());
    assert!(!or.parse("\"x\"").unwrap().is_right());
    assert!(!or.parse("1").unwrap().is_left());
    assert_eq!(
        issues(&or.parse("true").unwrap_err())[0],
        (String::new(), "invalid_union".into())
    );

    let s3 = vld::union3(vld::string(), vld::number().int(), vld::boolean());
    for ok in [json!("hi"), json!(42), json!(true)] {
        assert!(s3.parse_value(&ok).is_ok(), "{ok}");
    }
    assert_eq!(
        s3.parse_value(&json!(null)).unwrap_err().issues[0].code,
        IssueCode::InvalidUnion
    );

    let lit = vld::union(vld::literal("hello"), vld::literal(42i64));
    assert_eq!(
        lit.parse_value(&json!("hello")).unwrap().left().unwrap(),
        "hello"
    );
    assert_eq!(lit.parse_value(&json!(42)).unwrap().right().unwrap(), 42);
    assert_eq!(
        lit.parse_value(&json!("world")).unwrap_err().issues[0].code,
        IssueCode::InvalidUnion
    );

    let u = vld::union(vld::string().min(5), vld::number().min(10.0));
    let err = u.parse("3").unwrap_err();
    assert_eq!(
        issues(&err),
        pairs(&[
            ("", "invalid_union"),
            ("", "invalid_type"),
            ("", "too_small")
        ])
    );
    assert_eq!(err.issues[0].received, Some(json!(3)));
    assert!(err.issues[1].message.starts_with("[variant 1]"), "{err}");
    assert!(err.issues[2].message.starts_with("[variant 2]"), "{err}");

    let u3 = vld::union3(vld::string().min(2), vld::number().int(), vld::boolean());
    assert!(u3.schema_a().parse_value(&json!("a")).is_err());
    assert_eq!(u3.schema_b().parse_value(&json!(3)).unwrap(), 3);
    assert!(u3.schema_c().parse_value(&json!(true)).unwrap());
    let u2 = vld::union(vld::string(), vld::number());
    assert_eq!(u2.schema_a().parse_value(&json!("x")).unwrap(), "x");
    assert_eq!(u2.schema_b().parse_value(&json!(1.5)).unwrap(), 1.5);

    let s7 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
    );
    assert!(s7.parse("7").is_ok());
    assert!(s7.parse("8").is_err());
    let s8 = vld::union!(
        vld::literal("a"),
        vld::literal("b"),
        vld::literal("c"),
        vld::literal("d"),
        vld::literal("e"),
        vld::literal("f"),
        vld::literal("g"),
        vld::literal("h"),
    );
    assert!(s8.parse_value(&json!("h")).is_ok());
    assert!(s8.parse_value(&json!("z")).is_err());
    let s13 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
        vld::literal(8),
        vld::literal(9),
        vld::literal(10),
        vld::literal(11),
        vld::literal(12),
        vld::literal(13)
    );
    assert!(s13.parse("13").is_ok());
    let err = s13.parse("14").unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::InvalidUnion);
    let msgs: Vec<&str> = err.issues[1..].iter().map(|i| i.message.as_str()).collect();
    assert_eq!(msgs.len(), 13);
    for (i, m) in msgs.iter().enumerate() {
        assert_eq!(
            *m,
            format!("[variant {}] Expected literal {}", i + 1, i + 1)
        );
    }

    let both = vld::intersection(vld::string().min(3), vld::string().max(10));
    assert_eq!(both.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(
        issues(&both.parse(r#""hi""#).unwrap_err()),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(&both.parse(r#""hello world!""#).unwrap_err()),
        pairs(&[("", "too_big")])
    );
    let and = vld::string().min(3).and(vld::string().max(10));
    assert_eq!(
        issues(&and.parse(r#""hi""#).unwrap_err()),
        pairs(&[("", "too_small")])
    );
    assert!(and.parse(r#""hello""#).is_ok());
    let merged = vld::intersection(vld::string().min(100), vld::string().email());
    assert_eq!(
        issues(&merged.parse(r#""hi""#).unwrap_err()),
        pairs(&[("", "too_small"), ("", "invalid_string")])
    );
}

fn animal() -> ZDiscriminatedUnion {
    vld::discriminated_union("type")
        .variant_str(
            "dog",
            vld::object()
                .field("type", vld::literal("dog"))
                .field("bark", vld::boolean()),
        )
        .variant_str(
            "cat",
            vld::object()
                .field("type", vld::literal("cat"))
                .field("lives", vld::number().int()),
        )
}

/// Given a discriminated union of pets by `type` (and one keyed by the number `1`)
/// When bodies name a known, unknown or no variant
/// Then the chosen variant parses and only its errors are reported; an unknown tag is
/// `invalid_discriminator` at the tag with the options in insertion order, a missing tag is
/// `missing_field`, a non-object `invalid_type`; variants are listed in insertion order and each
/// validates alone; numeric tags compare by value (`1` selects variant `1.0`).
#[test]
fn pets_by_discriminator() {
    assert_eq!(
        animal().parse(r#"{"type":"dog","bark":true}"#).unwrap()["bark"],
        json!(true)
    );
    assert_eq!(
        animal().parse(r#"{"type":"cat","lives":9}"#).unwrap()["lives"],
        json!(9)
    );
    let err = animal().parse(r#"{"type":"cat","lives":"x"}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".lives", "invalid_type")]));

    let err = animal().parse(r#"{"type":"fish"}"#).unwrap_err();
    assert_eq!(issues(&err), pairs(&[(".type", "invalid_discriminator")]));
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidDiscriminator {
            options: vec![json!("dog"), json!("cat")]
        }
    );
    assert_eq!(
        issues(&animal().parse(r#"{"name":"x"}"#).unwrap_err()),
        pairs(&[(".type", "missing_field")])
    );
    assert_eq!(
        issues(&animal().parse_value(&json!("hello")).unwrap_err()),
        pairs(&[("", "invalid_type")])
    );

    let du = animal();
    let keys: Vec<serde_json::Value> = du.variants().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, [json!("dog"), json!("cat")]);
    let (_, cat) = du.variants().nth(1).unwrap();
    assert!(cat.dyn_parse(&json!({"type": "cat", "lives": 1})).is_ok());
    assert!(cat.dyn_parse(&json!({"type": "cat"})).is_err());

    let n = vld::discriminated_union("v").variant(1, vld::object().field("v", vld::number()));
    for input in [r#"{"v": 1.0}"#, r#"{"v": 1}"#] {
        assert_eq!(
            n.parse(input).unwrap().get("v"),
            Some(&json!(1.0)),
            "{input}"
        );
    }
    assert_eq!(
        issues(&n.parse(r#"{"v": 2}"#).unwrap_err()),
        pairs(&[(".v", "invalid_discriminator")])
    );
}
