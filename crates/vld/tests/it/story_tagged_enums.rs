//! Stories: `#[derive(Validate)]` on enums in every serde representation
//! (internally, externally, adjacently tagged, untagged), with flatten,
//! renames, generics, skipped variants and `deny_unknown_fields`. Every valid
//! input must give exactly the value serde gives, and the JSON Schema must
//! accept what serde writes.

use crate::common::*;

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vld::error::IssueCode;

/// Valid input: vld gives exactly what serde gives.
#[track_caller]
pub(crate) fn same_as_serde<T>(input: Value)
where
    T: vld::schema::VldParse + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let serde: T = serde_json::from_value(input.clone()).expect("serde accepts the input");
    let vld = T::vld_parse_value(&input).unwrap_or_else(|e| panic!("vld rejects {input}: {e}"));
    assert_eq!(vld, serde, "input {input}");
}

/// vld and serde agree on `input`; when both reject it, `unrecognized` lists
/// the paths vld reports as `unrecognized_field`.
#[track_caller]
pub(crate) fn agree<T>(input: Value, unrecognized: &[&str])
where
    T: vld::schema::VldParse + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let serde = serde_json::from_value::<T>(input.clone());
    let vld = T::vld_parse_value(&input);
    match (&serde, &vld) {
        (Ok(a), Ok(b)) => assert_eq!(a, b, "{input}"),
        (Err(_), Err(e)) => {
            let got: Vec<String> = e
                .issues
                .iter()
                .filter(|i| i.code.key() == "unrecognized_field")
                .map(|i| i.path_string())
                .collect();
            assert_eq!(got, unrecognized, "{input}: {e}");
        }
        _ => panic!("{input}: serde {serde:?} vs vld {vld:?}"),
    }
}

fn iss(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

#[cfg(feature = "openapi")]
fn validator(schema: &Value) -> jsonschema::Validator {
    jsonschema::draft202012::options()
        .build(schema)
        .unwrap_or_else(|e| panic!("invalid schema: {e}\n{schema:#}"))
}

vld::schema! {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Point {
        pub x: i64 => vld::number().int().min(0),
        pub y: i64 => vld::number().int().min(0),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Action {
    Ping,
    CreateUser {
        #[vld(vld::string().min(2))]
        name: String,
        #[vld(vld::string().email())]
        email: String,
        #[serde(default)]
        #[vld(vld::boolean())]
        admin: bool,
    },
    Move(Point),
    #[serde(rename = "delete", alias = "remove")]
    DeleteUser {
        #[vld(vld::number().int().min(1))]
        id: i64,
    },
}

/// Given an internally tagged `Action` (`{"type": ...}`) with a unit, struct,
/// newtype and renamed + aliased variant
/// When an API receives actions, valid and invalid, and validates instances
/// Then valid ones equal serde's value (extra keys, defaults, aliases too);
/// field issues sit at the variant's field paths, a missing / wrong-typed /
/// unknown tag is reported at `.type` (unknown lists every tag), a newtype
/// variant reports its inner struct's paths; `validate` checks an instance;
/// the JSON Schema is a `oneOf` with a discriminator, requires the tag and
/// links `Point` as a component.
#[test]
fn internally_tagged_actions() {
    same_as_serde::<Action>(json!({"type": "ping"}));
    same_as_serde::<Action>(json!({"type": "ping", "extra": 1}));
    same_as_serde::<Action>(json!({"type": "create_user", "name": "Al", "email": "a@b.co"}));
    same_as_serde::<Action>(
        json!({"type": "create_user", "name": "Al", "email": "a@b.co", "admin": true}),
    );
    same_as_serde::<Action>(json!({"type": "move", "x": 1, "y": 2}));
    same_as_serde::<Action>(json!({"type": "delete", "id": 3}));
    same_as_serde::<Action>(json!({"type": "remove", "id": 3}));

    let e = Action::parse_value(&json!({"type": "create_user", "name": "A", "email": "x"}))
        .unwrap_err();
    assert_eq!(
        issues(&e),
        iss(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let e = Action::parse_value(&json!({"name": "Al"})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".type", "missing_field")]));
    let e = Action::parse_value(&json!({"type": 5})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".type", "invalid_type")]));
    let e = Action::parse_value(&json!({"type": "fly"})).unwrap_err();
    assert_eq!(
        e.issues[0].code,
        IssueCode::InvalidDiscriminator {
            options: vec![
                json!("ping"),
                json!("create_user"),
                json!("move"),
                json!("delete")
            ]
        }
    );
    assert_eq!(issues(&e), iss(&[(".type", "invalid_discriminator")]));
    let e = Action::parse_value(&json!({"type": "move", "x": -1, "y": 1})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".x", "too_small")]));
    let e = Action::parse_value(&json!("ping")).unwrap_err();
    assert_eq!(issues(&e), iss(&[("", "invalid_type")]));

    assert!(Action::is_valid(&Action::CreateUser {
        name: "Al".into(),
        email: "a@b.co".into(),
        admin: false,
    }));
    let e = Action::validate(&Action::DeleteUser { id: 0 }).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".id", "too_small")]));

    #[cfg(feature = "openapi")]
    {
        let s = Action::json_schema();
        assert_eq!(s["discriminator"], json!({"propertyName": "type"}));
        assert_eq!(s["oneOf"].as_array().unwrap().len(), 4);
        let create = s["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["type"]["const"] == "create_user")
            .expect("create_user variant");
        assert_eq!(create["required"], json!(["type", "name", "email"]));
        let doc = Action::to_openapi_document();
        assert!(doc["components"]["schemas"]["Point"].is_object(), "{doc:#}");
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
enum Shape {
    Empty,
    Circle(#[vld(vld::number().positive())] f64),
    Rect(
        #[vld(vld::number().int().min(1))] i64,
        #[vld(vld::number().int().min(1))] i64,
    ),
    Labeled {
        #[vld(vld::string().non_empty())]
        label: String,
    },
}

/// Given an externally tagged `Shape` (serde's default: `"Empty"`,
/// `{"Circle": 1.5}`, `{"Rect": [w, h]}`, `{"Labeled": {...}}`)
/// When shapes are parsed
/// Then valid ones equal serde's value; issues sit under the variant key
/// (`.Circle`, `.Rect[1]`, `.Labeled.label`), a short tuple is
/// `invalid_length`, a unit variant with content is `invalid_type`, an unknown
/// name is `invalid_discriminator`, two keys are `invalid_type`; the JSON Schema
/// accepts every serialized shape and rejects the invalid ones.
#[test]
fn externally_tagged_shapes() {
    same_as_serde::<Shape>(json!("Empty"));
    same_as_serde::<Shape>(json!({"Empty": null}));
    same_as_serde::<Shape>(json!({"Circle": 1.5}));
    same_as_serde::<Shape>(json!({"Rect": [2, 3]}));
    same_as_serde::<Shape>(json!({"Labeled": {"label": "x"}}));

    let cases = [
        (json!({"Circle": -1}), ".Circle", "too_small"),
        (json!({"Rect": [2, 0]}), ".Rect[1]", "too_small"),
        (json!({"Rect": [2]}), ".Rect", "invalid_length"),
        (
            json!({"Labeled": {"label": ""}}),
            ".Labeled.label",
            "too_small",
        ),
        (json!({"Empty": 1}), ".Empty", "invalid_type"),
        (json!("Square"), "", "invalid_discriminator"),
        (json!({"Circle": 1, "Empty": null}), "", "invalid_type"),
    ];
    for (input, path, code) in cases {
        let e = Shape::parse_value(&input).unwrap_err();
        assert_eq!(issues(&e), iss(&[(path, code)]), "{input}");
    }

    #[cfg(feature = "openapi")]
    {
        let shape = validator(&Shape::json_schema());
        for v in [
            Shape::Empty,
            Shape::Circle(1.0),
            Shape::Rect(1, 2),
            Shape::Labeled { label: "x".into() },
        ] {
            assert!(shape.is_valid(&serde_json::to_value(&v).unwrap()), "{v:?}");
        }
        for bad in [json!("Square"), json!({"Circle": -1}), json!({"Rect": [1]})] {
            assert!(!shape.is_valid(&bad), "{bad}");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(
    tag = "t",
    content = "c",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum Event {
    Started,
    Progress(#[vld(vld::number().int().min(0).max(100))] i64),
    Pair(
        #[vld(vld::string())] String,
        #[vld(vld::number().int())] i64,
    ),
    UserJoined {
        #[vld(vld::string().min(1))]
        user_name: String,
    },
    #[serde(other)]
    Unknown,
}

/// Given an adjacently tagged `Event` (`{"t": ..., "c": ...}`) with
/// `rename_all` / `rename_all_fields` and a `#[serde(other)]` fallback
/// When events are parsed
/// Then valid ones (an unknown tag included) equal serde's value; issues sit
/// under `.c` with the renamed field names (`.c.userName`), missing content is
/// `missing_field` at `.c`; the JSON Schema accepts serialized events and
/// rejects an out-of-range progress.
#[test]
fn adjacently_tagged_events() {
    same_as_serde::<Event>(json!({"t": "started"}));
    same_as_serde::<Event>(json!({"t": "progress", "c": 50}));
    same_as_serde::<Event>(json!({"t": "pair", "c": ["a", 1]}));
    same_as_serde::<Event>(json!({"t": "user-joined", "c": {"userName": "x"}}));
    same_as_serde::<Event>(json!({"t": "something-new"}));

    let cases = [
        (json!({"t": "progress", "c": 101}), ".c", "too_big"),
        (json!({"t": "progress"}), ".c", "missing_field"),
        (
            json!({"t": "user-joined", "c": {"user_name": "x"}}),
            ".c.userName",
            "missing_field",
        ),
        (
            json!({"t": "pair", "c": ["a", "b"]}),
            ".c[1]",
            "invalid_type",
        ),
    ];
    for (input, path, code) in cases {
        let e = Event::parse_value(&input).unwrap_err();
        assert_eq!(issues(&e), iss(&[(path, code)]), "{input}");
    }

    #[cfg(feature = "openapi")]
    {
        let event = validator(&Event::json_schema());
        for v in [
            Event::Started,
            Event::Progress(5),
            Event::Pair("a".into(), 1),
            Event::UserJoined {
                user_name: "x".into(),
            },
        ] {
            assert!(event.is_valid(&serde_json::to_value(&v).unwrap()), "{v:?}");
        }
        assert!(!event.is_valid(&json!({"t": "progress", "c": 500})));
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(untagged)]
enum Id {
    Num(#[vld(vld::number().int().min(1))] i64),
    Name(#[vld(vld::string().min(3))] String),
    Nothing,
}

/// Given an untagged `Id` (a positive number, a 3+ char name, or `null`)
/// When ids are parsed
/// Then valid ones equal serde's value; a value no variant takes is
/// `invalid_union` first, followed by each variant's issues, their messages
/// prefixed `[variant N]`; the JSON Schema agrees.
#[test]
fn untagged_ids_try_variants_in_order() {
    same_as_serde::<Id>(json!(5));
    same_as_serde::<Id>(json!("abc"));
    same_as_serde::<Id>(json!(null));

    let e = Id::parse_value(&json!("ab")).unwrap_err();
    let got = issues(&e);
    assert_eq!(got[0], (String::new(), "invalid_union".into()));
    assert!(
        got.contains(&(String::new(), "too_small".into())),
        "{got:?}"
    );
    assert!(
        e.issues
            .iter()
            .any(|i| i.message.starts_with("[variant 2]"))
    );

    #[cfg(feature = "openapi")]
    {
        let id = validator(&Id::json_schema());
        for ok in [json!(1), json!("abc"), json!(null)] {
            assert!(id.is_valid(&ok), "{ok}");
        }
        assert!(!id.is_valid(&json!("ab")));
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "kind")]
enum Renamed {
    #[serde(rename(serialize = "out", deserialize = "in"))]
    Both {
        #[vld(vld::number().int())]
        n: i64,
    },
    #[serde(skip)]
    #[allow(dead_code)]
    Internal,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "type")]
enum Wrapper<T: vld::schema::VldParse + Send + Sync + 'static> {
    One {
        #[vld(vld::nested(T::vld_parse_value))]
        item: T,
    },
}

/// Given a variant renamed differently for serialize / deserialize, a skipped
/// variant, and a generic enum wrapping a nested schema type
/// When values are parsed and instances validated
/// Then parsing reads the deserialize name only, `validate` uses the serialize
/// name serde writes, a skipped variant is an unknown tag, and the generic
/// enum reports the nested field at `.item.x`.
#[test]
fn split_renames_skipped_variants_and_generic_enums() {
    same_as_serde::<Renamed>(json!({"kind": "in", "n": 1}));
    assert!(Renamed::parse_value(&json!({"kind": "out", "n": 1})).is_err());
    assert!(Renamed::is_valid(&Renamed::Both { n: 1 }));
    let e = Renamed::parse_value(&json!({"kind": "Internal"})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".kind", "invalid_discriminator")]));

    same_as_serde::<Wrapper<Point>>(json!({"type": "One", "item": {"x": 1, "y": 2}}));
    let e = Wrapper::<Point>::parse_value(&json!({"type": "One", "item": {"x": -1, "y": 2}}))
        .unwrap_err();
    assert_eq!(issues(&e), iss(&[(".item.x", "too_small")]));
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, vld::Validate)]
struct Meta {
    #[vld(vld::string().min(1))]
    author: String,
    #[vld(vld::number().int().min(1).optional())]
    version: Option<i64>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "type")]
enum Post {
    Article {
        #[vld(vld::string())]
        body: String,
        #[serde(flatten)]
        meta: Meta,
    },
}

/// Given a struct variant with a `#[serde(flatten)]`ed derived struct
/// When an article is parsed with and without the flattened keys
/// Then it equals serde's value, and a missing flattened key is reported at
/// its own top-level path (`.author`).
#[test]
fn flatten_inside_a_struct_variant() {
    same_as_serde::<Post>(json!({"type": "Article", "body": "b", "author": "a"}));
    let e = Post::parse_value(&json!({"type": "Article", "body": "b"})).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".author", "missing_field")]));
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Act {
    Sow,
    Plant {
        #[vld(vld::string())]
        seed: String,
    },
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "t", content = "c", deny_unknown_fields)]
enum Adj {
    A,
    B {
        #[vld(vld::number().int())]
        n: i64,
    },
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(deny_unknown_fields)]
enum Ext {
    S {
        #[vld(vld::number().int())]
        n: i64,
    },
}

/// Given enums with `#[serde(deny_unknown_fields)]` in internal, adjacent and
/// external form
/// When inputs with and without extra keys are parsed
/// Then vld rejects exactly what serde rejects, reporting each extra key as
/// `unrecognized_field` at its path (`.extra`, `.c.m`, `.x`, `.S.z`); on a
/// unit variant of an internally tagged enum vld is deliberately stricter
/// than serde (which ignores the attribute there); every variant's schema has
/// `additionalProperties: false`, and a validator rejects the extra key.
#[test]
fn deny_unknown_fields_on_enums() {
    agree::<Act>(json!({"type": "sow"}), &[]);
    agree::<Act>(json!({"type": "plant", "seed": "x"}), &[]);
    agree::<Act>(
        json!({"type": "plant", "seed": "x", "extra": 1}),
        &[".extra"],
    );
    agree::<Adj>(json!({"t": "B", "c": {"n": 1}}), &[]);
    agree::<Adj>(json!({"t": "B", "c": {"n": 1, "m": 2}}), &[".c.m"]);
    agree::<Adj>(json!({"t": "A", "x": 1}), &[".x"]);
    agree::<Ext>(json!({"S": {"n": 1}}), &[]);
    agree::<Ext>(json!({"S": {"n": 1, "z": 0}}), &[".S.z"]);

    let input = json!({"type": "sow", "extra": 1});
    assert!(serde_json::from_value::<Act>(input.clone()).is_ok());
    let e = Act::parse_value(&input).unwrap_err();
    assert_eq!(issues(&e), iss(&[(".extra", "unrecognized_field")]));

    #[cfg(feature = "openapi")]
    {
        let s = Act::json_schema();
        for v in s["oneOf"].as_array().unwrap() {
            assert_eq!(v["additionalProperties"], false, "{v}");
        }
        let v = validator(&s);
        assert!(v.is_valid(&json!({"type": "plant", "seed": "x"})));
        assert!(!v.is_valid(&json!({"type": "plant", "seed": "x", "extra": 1})));
    }
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        Just(Action::Ping),
        ("[a-z]{2,8}", "[a-z]{1,5}@[a-z]{1,5}\\.co", any::<bool>())
            .prop_map(|(name, email, admin)| Action::CreateUser { name, email, admin }),
        (0..1000i64, 0..1000i64).prop_map(|(x, y)| Action::Move(Point { x, y })),
        (1..1000i64).prop_map(|id| Action::DeleteUser { id }),
    ]
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        Just(Event::Started),
        (0..=100i64).prop_map(Event::Progress),
        ("[a-z]{0,5}", any::<i64>()).prop_map(|(s, n)| Event::Pair(s, n)),
        "[a-z]{1,8}".prop_map(|user_name| Event::UserJoined { user_name }),
    ]
}

proptest! {
    /// Given any valid `Action` / `Event` value
    /// When serde serializes it and vld parses the JSON back
    /// Then vld returns the same value.
    #[test]
    fn whatever_serde_writes_vld_parses_back(a in action(), e in event()) {
        let v = serde_json::to_value(&a).unwrap();
        prop_assert_eq!(Action::parse_value(&v).unwrap(), a);
        let v = serde_json::to_value(&e).unwrap();
        prop_assert_eq!(Event::parse_value(&v).unwrap(), e);
    }
}
