//! `#[derive(Validate)]` on enums: every serde representation, checked
//! against serde itself (valid input must give the value serde gives).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vld::error::{IssueCode, PathSegment, VldError};

/// `(path, code key)` of every issue, path as `.a[0].b`.
fn issues(e: &VldError) -> Vec<(String, String)> {
    e.issues
        .iter()
        .map(|i| {
            let path: String = i
                .path
                .iter()
                .map(|s| match s {
                    PathSegment::Field(f) => format!(".{f}"),
                    PathSegment::Index(n) => format!("[{n}]"),
                })
                .collect();
            (path, i.code.key().to_string())
        })
        .collect()
}

/// Valid input: vld gives exactly what serde gives.
fn same_as_serde<T>(input: Value)
where
    T: vld::schema::VldParse + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let serde: T = serde_json::from_value(input.clone()).expect("serde accepts the input");
    let vld = T::vld_parse_value(&input).unwrap_or_else(|e| panic!("vld rejects {input}: {e}"));
    assert_eq!(vld, serde, "input {input}");
}

vld::schema! {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Point {
        pub x: i64 => vld::number().int().min(0),
        pub y: i64 => vld::number().int().min(0),
    }
}

// ---------------------------------------------------------------------------
// Internally tagged: the shape of a typical `Action`
// ---------------------------------------------------------------------------

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

#[test]
fn internal_valid_inputs_match_serde() {
    same_as_serde::<Action>(json!({"type": "ping"}));
    same_as_serde::<Action>(json!({"type": "ping", "extra": 1}));
    same_as_serde::<Action>(json!({"type": "create_user", "name": "Al", "email": "a@b.co"}));
    same_as_serde::<Action>(
        json!({"type": "create_user", "name": "Al", "email": "a@b.co", "admin": true}),
    );
    same_as_serde::<Action>(json!({"type": "move", "x": 1, "y": 2}));
    same_as_serde::<Action>(json!({"type": "delete", "id": 3}));
    same_as_serde::<Action>(json!({"type": "remove", "id": 3}));
}

#[test]
fn internal_errors() {
    let e = Action::parse_value(&json!({"type": "create_user", "name": "A", "email": "x"}))
        .unwrap_err();
    assert_eq!(
        issues(&e),
        [
            (".name".into(), "too_small".into()),
            (".email".into(), "invalid_string".into())
        ]
    );

    let e = Action::parse_value(&json!({"name": "Al"})).unwrap_err();
    assert_eq!(issues(&e), [(".type".into(), "missing_field".into())]);

    let e = Action::parse_value(&json!({"type": 5})).unwrap_err();
    assert_eq!(issues(&e), [(".type".into(), "invalid_type".into())]);

    let e = Action::parse_value(&json!({"type": "fly"})).unwrap_err();
    assert_eq!(
        issues(&e),
        [(".type".into(), "invalid_discriminator".into())]
    );
    match &e.issues[0].code {
        IssueCode::InvalidDiscriminator { options } => assert_eq!(
            options,
            &[
                json!("ping"),
                json!("create_user"),
                json!("move"),
                json!("delete")
            ]
        ),
        other => panic!("{other:?}"),
    }

    // Newtype variant: the inner struct's issues, at its own paths.
    let e = Action::parse_value(&json!({"type": "move", "x": -1, "y": 1})).unwrap_err();
    assert_eq!(issues(&e), [(".x".into(), "too_small".into())]);

    let e = Action::parse_value(&json!("ping")).unwrap_err();
    assert_eq!(issues(&e), [("".into(), "invalid_type".into())]);
}

#[test]
fn internal_validate_checks_an_instance() {
    let ok = Action::CreateUser {
        name: "Al".into(),
        email: "a@b.co".into(),
        admin: false,
    };
    assert!(Action::is_valid(&ok));
    let bad = Action::DeleteUser { id: 0 };
    let e = Action::validate(&bad).unwrap_err();
    assert_eq!(issues(&e), [(".id".into(), "too_small".into())]);
}

// ---------------------------------------------------------------------------
// Externally tagged (serde's default)
// ---------------------------------------------------------------------------

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

#[test]
fn external_valid_inputs_match_serde() {
    same_as_serde::<Shape>(json!("Empty"));
    same_as_serde::<Shape>(json!({"Empty": null}));
    same_as_serde::<Shape>(json!({"Circle": 1.5}));
    same_as_serde::<Shape>(json!({"Rect": [2, 3]}));
    same_as_serde::<Shape>(json!({"Labeled": {"label": "x"}}));
}

#[test]
fn external_errors() {
    let e = Shape::parse_value(&json!({"Circle": -1})).unwrap_err();
    assert_eq!(issues(&e), [(".Circle".into(), "too_small".into())]);

    let e = Shape::parse_value(&json!({"Rect": [2, 0]})).unwrap_err();
    assert_eq!(issues(&e), [(".Rect[1]".into(), "too_small".into())]);

    let e = Shape::parse_value(&json!({"Rect": [2]})).unwrap_err();
    assert_eq!(issues(&e), [(".Rect".into(), "invalid_length".into())]);

    let e = Shape::parse_value(&json!({"Labeled": {"label": ""}})).unwrap_err();
    assert_eq!(issues(&e), [(".Labeled.label".into(), "too_small".into())]);

    let e = Shape::parse_value(&json!({"Empty": 1})).unwrap_err();
    assert_eq!(issues(&e), [(".Empty".into(), "invalid_type".into())]);

    let e = Shape::parse_value(&json!("Square")).unwrap_err();
    assert_eq!(issues(&e), [("".into(), "invalid_discriminator".into())]);

    let e = Shape::parse_value(&json!({"Circle": 1, "Empty": null})).unwrap_err();
    assert_eq!(issues(&e), [("".into(), "invalid_type".into())]);
}

// ---------------------------------------------------------------------------
// Adjacently tagged
// ---------------------------------------------------------------------------

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

#[test]
fn adjacent_valid_inputs_match_serde() {
    same_as_serde::<Event>(json!({"t": "started"}));
    same_as_serde::<Event>(json!({"t": "progress", "c": 50}));
    same_as_serde::<Event>(json!({"t": "pair", "c": ["a", 1]}));
    same_as_serde::<Event>(json!({"t": "user-joined", "c": {"userName": "x"}}));
    same_as_serde::<Event>(json!({"t": "something-new"}));
}

#[test]
fn adjacent_errors() {
    let e = Event::parse_value(&json!({"t": "progress", "c": 101})).unwrap_err();
    assert_eq!(issues(&e), [(".c".into(), "too_big".into())]);

    let e = Event::parse_value(&json!({"t": "progress"})).unwrap_err();
    assert_eq!(issues(&e), [(".c".into(), "missing_field".into())]);

    let e = Event::parse_value(&json!({"t": "user-joined", "c": {"user_name": "x"}})).unwrap_err();
    assert_eq!(issues(&e), [(".c.userName".into(), "missing_field".into())]);

    let e = Event::parse_value(&json!({"t": "pair", "c": ["a", "b"]})).unwrap_err();
    assert_eq!(issues(&e), [(".c[1]".into(), "invalid_type".into())]);
}

// ---------------------------------------------------------------------------
// Untagged
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(untagged)]
enum Id {
    Num(#[vld(vld::number().int().min(1))] i64),
    Name(#[vld(vld::string().min(3))] String),
    Nothing,
}

#[test]
fn untagged_tries_variants_in_order() {
    same_as_serde::<Id>(json!(5));
    same_as_serde::<Id>(json!("abc"));
    same_as_serde::<Id>(json!(null));

    let e = Id::parse_value(&json!("ab")).unwrap_err();
    let got = issues(&e);
    assert_eq!(got[0], ("".into(), "invalid_union".into()));
    assert!(got.contains(&("".into(), "too_small".into())), "{got:?}");
    assert!(
        e.issues
            .iter()
            .any(|i| i.message.starts_with("[variant 2]"))
    );
}

// ---------------------------------------------------------------------------
// serialize names, generics, skipped variants
// ---------------------------------------------------------------------------

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

#[test]
fn serialize_names_are_used_by_validate() {
    same_as_serde::<Renamed>(json!({"kind": "in", "n": 1}));
    assert!(Renamed::parse_value(&json!({"kind": "out", "n": 1})).is_err());
    assert!(Renamed::is_valid(&Renamed::Both { n: 1 }));
    // A skipped variant is not accepted.
    let e = Renamed::parse_value(&json!({"kind": "Internal"})).unwrap_err();
    assert_eq!(
        issues(&e),
        [(".kind".into(), "invalid_discriminator".into())]
    );
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
#[serde(tag = "type")]
enum Wrapper<T: vld::schema::VldParse + Send + Sync + 'static> {
    One {
        #[vld(vld::nested(T::vld_parse_value))]
        item: T,
    },
}

#[test]
fn generic_enum() {
    same_as_serde::<Wrapper<Point>>(json!({"type": "One", "item": {"x": 1, "y": 2}}));
    let e = Wrapper::<Point>::parse_value(&json!({"type": "One", "item": {"x": -1, "y": 2}}))
        .unwrap_err();
    assert_eq!(issues(&e), [(".item.x".into(), "too_small".into())]);
}

// ---------------------------------------------------------------------------
// Round trip: whatever serde serializes, vld parses back to the same value
// ---------------------------------------------------------------------------

mod round_trip {
    use super::*;
    use proptest::prelude::*;

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
        #[test]
        fn action_round_trips(a in action()) {
            let v = serde_json::to_value(&a).unwrap();
            prop_assert_eq!(Action::parse_value(&v).unwrap(), a);
        }

        #[test]
        fn event_round_trips(e in event()) {
            let v = serde_json::to_value(&e).unwrap();
            prop_assert_eq!(Event::parse_value(&v).unwrap(), e);
        }
    }
}

// ---------------------------------------------------------------------------
// JSON Schema: a real validator accepts what serde writes, rejects the rest
// ---------------------------------------------------------------------------

#[cfg(feature = "openapi")]
mod schema {
    use super::*;

    fn validator(schema: &Value) -> jsonschema::Validator {
        jsonschema::draft202012::options()
            .build(schema)
            .unwrap_or_else(|e| panic!("invalid schema: {e}\n{schema:#}"))
    }

    #[test]
    fn internal_schema() {
        let s = Action::json_schema();
        assert_eq!(s["discriminator"], json!({"propertyName": "type"}));
        assert_eq!(s["oneOf"].as_array().unwrap().len(), 4);
        // `Move(Point)` points at the nested component.
        let doc = Action::to_openapi_document();
        assert!(doc["components"]["schemas"]["Point"].is_object(), "{doc:#}");
    }

    #[test]
    fn schemas_accept_serde_output_and_reject_invalid() {
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

        let id = validator(&Id::json_schema());
        for ok in [json!(1), json!("abc"), json!(null)] {
            assert!(id.is_valid(&ok), "{ok}");
        }
        assert!(!id.is_valid(&json!("ab")));
    }

    #[test]
    fn internal_struct_variant_schema_requires_the_tag() {
        let s = Action::json_schema();
        let create = s["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["type"]["const"] == "create_user")
            .expect("create_user variant");
        assert_eq!(create["required"], json!(["type", "name", "email"]));
    }
}

// ---------------------------------------------------------------------------
// #[serde(flatten)]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, vld::Validate)]
struct Meta {
    #[vld(vld::string().min(1))]
    author: String,
    #[vld(vld::number().int().min(1).optional())]
    version: Option<i64>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, vld::Validate)]
struct Doc {
    #[vld(vld::string().min(1))]
    title: String,
    #[serde(flatten)]
    meta: Meta,
    #[serde(flatten)]
    #[vld(vld::nested(Point::parse_value))]
    at: Point,
}

#[test]
fn flatten_matches_serde() {
    same_as_serde::<Doc>(json!({"title": "t", "author": "a", "x": 1, "y": 2}));
    same_as_serde::<Doc>(json!({"title": "t", "author": "a", "version": 3, "x": 1, "y": 2}));
    let d = Doc {
        title: "t".into(),
        meta: Meta {
            author: "a".into(),
            version: None,
        },
        at: Point { x: 0, y: 0 },
    };
    assert!(Doc::is_valid(&d));
}

#[test]
fn flatten_errors_keep_the_inner_paths() {
    let e = Doc::parse_value(&json!({"title": "", "author": "", "x": -1, "y": 0})).unwrap_err();
    assert_eq!(
        issues(&e),
        [
            (".title".into(), "too_small".into()),
            (".author".into(), "too_small".into()),
            (".x".into(), "too_small".into()),
        ]
    );
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

#[test]
fn flatten_in_struct_variant() {
    same_as_serde::<Post>(json!({"type": "Article", "body": "b", "author": "a"}));
    let e = Post::parse_value(&json!({"type": "Article", "body": "b"})).unwrap_err();
    assert_eq!(issues(&e), [(".author".into(), "missing_field".into())]);
}

#[cfg(feature = "openapi")]
#[test]
fn flatten_merges_the_schema() {
    let s = Doc::json_schema();
    let props: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
    // A flattened derived type is inlined; an opaque explicit schema
    // (`vld::nested(..)`) cannot be, so it goes into allOf.
    for k in ["title", "author", "version"] {
        assert!(props.iter().any(|p| *p == k), "{k} missing in {s:#}");
    }
    assert_eq!(s["allOf"], json!([{"type": "object"}]));
    let required: Vec<&str> = s["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(required, ["title", "author"]);
    let v = jsonschema::draft202012::options().build(&s).unwrap();
    assert!(v.is_valid(&json!({"title": "t", "author": "a", "x": 1, "y": 2})));
    assert!(!v.is_valid(&json!({"title": "t", "x": 1, "y": 2})));
}
