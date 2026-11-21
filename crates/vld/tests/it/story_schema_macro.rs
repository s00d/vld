//! Stories: types declared with `vld::schema!` / `schema_validated!`, the
//! helper macros `impl_validate_fields!`, `impl_rules!`, `impl_default!` and
//! `nested!`: parse, read issues, validate instances, parse leniently, emit
//! OpenAPI components. Whole emitted documents are snapshots in
//! `corpus_json_schema.rs`.

use crate::common::*;

use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

fn iss(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

fn paths(e: &vld::error::VldError) -> Vec<String> {
    e.issues.iter().map(path_of).collect()
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct User {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
        age: Option<i64> => vld::number().int().min(18).optional(),
    }
}

/// Given a `User` declared with `schema!`, a `Person` nesting an `Address`
/// through `vld::nested`, and a config with an enumeration field
/// When sign-up bodies are parsed (from text and from JSON values) and
/// existing instances / raw values validated
/// Then valid bodies give the typed struct (absent optional = `None`); a bad
/// body reports every field in order; a non-object is `invalid_type`
/// (expected `object`); nested issues carry the full path
/// (`.address.city`); `validate` / `is_valid` check instances and raw values
/// the same way.
#[test]
fn sign_up_form_declared_with_schema_macro() {
    let u = User::parse(r#"{"name": "Alex", "email": "a@b.co", "age": 25}"#).unwrap();
    assert_eq!((u.name.as_str(), u.age), ("Alex", Some(25)));
    let u = User::parse(&json!({"name": "Bob", "email": "b@c.co"})).unwrap();
    assert_eq!(u.age, None);

    let err = User::parse(r#"{"name": "A", "email": "bad", "age": 5}"#).unwrap_err();
    assert_eq!(
        issues(&err),
        iss(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_small")
        ])
    );
    assert_eq!(
        User::parse("[1]").unwrap_err().issues[0].code,
        IssueCode::InvalidType {
            expected: "object".into(),
            received: "array".into()
        }
    );

    let good = User::parse(r#"{"name": "Alice", "email": "a@b.com", "age": 25}"#).unwrap();
    assert!(User::is_valid(&good));
    assert!(User::validate(&good).is_ok());
    let bad = User {
        name: "A".into(),
        email: "bad".into(),
        age: Some(10),
    };
    assert!(!User::is_valid(&bad));
    assert_eq!(
        paths(&User::validate(&bad).unwrap_err()),
        [".name", ".email", ".age"]
    );
    assert!(User::is_valid(&json!({"name": "Bob", "email": "b@t.co"})));

    vld::schema! {
        #[derive(Debug)]
        struct Address {
            city: String => vld::string().min(1),
        }
    }
    vld::schema! {
        #[derive(Debug)]
        struct Person {
            name: String => vld::string(),
            address: Address => vld::nested(Address::parse_value),
        }
    }
    let p = Person::parse(r#"{"name": "Alex", "address": {"city": "Moscow"}}"#).unwrap();
    assert_eq!(p.address.city, "Moscow");
    let err = Person::parse(r#"{"name": "Alex", "address": {"city": ""}}"#).unwrap_err();
    assert_eq!(issues(&err), iss(&[(".address.city", "too_small")]));

    vld::schema! {
        #[derive(Debug)]
        struct Mode {
            mode: String => vld::enumeration(&["dev", "prod"]),
        }
    }
    assert_eq!(Mode::parse(r#"{"mode": "dev"}"#).unwrap().mode, "dev");
    assert_eq!(
        issues(&Mode::parse(r#"{"mode": "x"}"#).unwrap_err()),
        iss(&[(".mode", "invalid_enum_value")])
    );
}

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct RenamedUser {
        pub first_name: String as "firstName" => vld::string().min(1),
        pub last_name: String as "lastName" => vld::string().min(1),
        pub email_address: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct RawIdent {
        pub r#type: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Camel {
        first_name: String => vld::string().min(1),
        #[serde(rename = "years")]
        age_years: i64 => vld::i64().min(0),
        #[serde(rename(serialize = "outName", deserialize = "inName"))]
        split_name: String => vld::string(),
    }
}

/// One field `user_name` under a serde `rename_all` convention.
macro_rules! rename_all_case {
    ($name:ident, $rule:literal) => {
        vld::schema! {
            #[derive(Debug, Default, serde::Serialize)]
            #[serde(rename_all = $rule)]
            struct $name {
                user_name: String => vld::string(),
            }
        }
    };
}
rename_all_case!(Lower, "lowercase");
rename_all_case!(Upper, "UPPERCASE");
rename_all_case!(Screaming, "SCREAMING_SNAKE_CASE");
rename_all_case!(Pascal, "PascalCase");
rename_all_case!(Kebab, "kebab-case");
rename_all_case!(ScreamingKebab, "SCREAMING-KEBAB-CASE");

vld::schema! {
    #[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Tricky {
        #[serde(rename(serialize = "split_out", deserialize = "split_in"), default)]
        split: String => vld::string(),
        #[serde(rename = "comma,(", default)]
        comma: String => vld::string(),
        #[serde(rename = "close)", default)]
        close: String => vld::string(),
        #[serde(rename = "quote\",x", default)]
        quote: String => vld::string(),
        #[serde(rename = "ctrl\n\t\r\0\\", default)]
        ctrl: String => vld::string(),
    }
}

/// Given `schema!` types renamed with `field as "jsonKey"`, a raw identifier
/// `r#type`, and serde `rename_all` / `rename` / split serialize-deserialize
/// renames
/// When bodies are parsed, instances validated and the JSON Schema emitted
/// Then parsing reads only the JSON keys and reports issues under them (Rust
/// names are not read), `r#type` is the key `type`, `validate` accepts an
/// instance (serialize names), and the schema's properties and `required`
/// list use the JSON keys.
#[test]
fn renamed_keys_in_and_out() {
    let u = RenamedUser::parse(
        r#"{"firstName": "John", "lastName": "Doe", "email_address": "j@d.co"}"#,
    )
    .unwrap();
    assert_eq!(
        (u.first_name.as_str(), u.last_name.as_str()),
        ("John", "Doe")
    );
    let err = RenamedUser::parse(
        r#"{"first_name": "John", "last_name": "Doe", "email_address": "j@d.co"}"#,
    )
    .unwrap_err();
    assert_eq!(paths(&err), [".firstName", ".lastName"]);
    let u = <RenamedUser as VldParse>::vld_parse_value(&json!({
        "firstName": "Jane", "lastName": "S", "email_address": "j@s.co"
    }))
    .unwrap();
    assert_eq!(u.first_name, "Jane");

    assert_eq!(
        RawIdent::parse_value(&json!({"type": "a"})).unwrap().r#type,
        "a"
    );
    assert_eq!(
        paths(&RawIdent::parse_value(&json!({})).unwrap_err()),
        [".type"]
    );

    let v = Camel::parse(r#"{"firstName": "A", "years": 3, "inName": "x"}"#).unwrap();
    assert_eq!(v.first_name, "A");
    assert!(Camel::validate(&v).is_ok());
    assert_eq!(serde_json::to_value(&v).unwrap()["outName"], json!("x"));
    let err = Camel::parse(r#"{"first_name": "A", "years": 3, "inName": "x"}"#).unwrap_err();
    assert_eq!(paths(&err), [".firstName"]);

    // Every `rename_all` convention gives the key serde writes.
    fn serde_key<T: serde::Serialize + Default>() -> String {
        let v = serde_json::to_value(T::default()).unwrap();
        v.as_object().unwrap().keys().next().unwrap().clone()
    }
    macro_rules! same_key_as_serde {
        ($($t:ident => $key:literal),+) => {$(
            assert_eq!(serde_key::<$t>(), $key);
            assert!($t::parse_value(&json!({ $key: "v" })).is_ok(), $key);
            assert_eq!(paths(&$t::parse_value(&json!({})).unwrap_err()), [concat!(".", $key)]);
        )+};
    }
    same_key_as_serde!(
        Lower => "user_name",
        Upper => "USER_NAME",
        Screaming => "USER_NAME",
        Pascal => "UserName",
        Kebab => "user-name",
        ScreamingKebab => "USER-NAME"
    );

    // Renames with commas, parentheses and escaped quotes inside the key, next to other
    // serde options: vld reads and writes the keys serde does.
    let body = json!({"split_in": "s", "comma,(": "c", "close)": "p", "quote\",x": "q",
                     "ctrl\n\t\r\0\\": "z"});
    let t = Tricky::parse_value(&body).unwrap();
    assert_eq!(t, serde_json::from_value::<Tricky>(body).unwrap());
    assert_eq!(
        (
            t.split.as_str(),
            t.comma.as_str(),
            t.close.as_str(),
            t.quote.as_str()
        ),
        ("s", "c", "p", "q")
    );
    assert!(Tricky::validate(&t).is_ok());
    let out = serde_json::to_value(&t).unwrap();
    let mut out_keys: Vec<&String> = out.as_object().unwrap().keys().collect();
    out_keys.sort();
    assert_eq!(
        out_keys,
        [
            "close)",
            "comma,(",
            "ctrl\n\t\r\0\\",
            "quote\",x",
            "split_out"
        ]
    );
    assert_eq!(t.ctrl, "z");

    #[cfg(feature = "openapi")]
    {
        let js = RenamedUser::json_schema();
        let mut props: Vec<&String> = js["properties"].as_object().unwrap().keys().collect();
        props.sort();
        assert_eq!(props, ["email_address", "firstName", "lastName"]);
        assert_eq!(
            js["required"],
            json!(["firstName", "lastName", "email_address"])
        );
        let props = &Camel::json_schema()["properties"];
        for k in ["firstName", "years", "inName"] {
            assert!(props.get(k).is_some(), "{k}");
        }
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize, Default, PartialEq)]
    struct Inner {
        city: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct Profile {
        name: String        => vld::string().min(2).max(50),
        email: String       => vld::string().email(),
        age: Option<i64>    => vld::number().int().gte(18).optional(),
        role: String        => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
        nick: String        => vld::string().min(3).catch("anon".to_string()),
        inner: Inner        => vld::nested(Inner::parse_value),
    }
}

vld::impl_validate_fields!(Profile {
    name  : String      => vld::string().min(2).max(50),
    email : String      => vld::string().email(),
    age   : Option<i64> => vld::number().int().gte(18).optional(),
    role  : String      => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
    nick  : String      => vld::string().min(3).catch("anon".to_string()),
    inner : Inner       => vld::nested(Inner::parse_value),
});

vld::schema! {
    #[derive(Debug, Default, serde::Serialize)]
    pub struct RenamedProfile {
        pub user_name: String as "userName" => vld::string().min(2),
        pub age: Option<i64> => vld::number().int().min(0).optional(),
    }
}

vld::impl_validate_fields!(RenamedProfile {
    user_name: String as "userName" => vld::string().min(2),
    age: Option<i64> => vld::number().int().min(0).optional(),
});

fn profile(name: &str, email: &str, nick: &str) -> serde_json::Value {
    json!({"name": name, "email": email, "age": 25, "role": "admin", "nick": nick, "inner": {"city": "NY"}})
}

/// Given a `Profile` with `impl_validate_fields!` (string bounds, email,
/// optional age, defaulted role, a `catch` nick, a nested struct)
/// When profiles are imported from JSON values and files, checked field by
/// field and parsed leniently, and the result is printed, saved and split
/// Then `validate_fields` gives one result per field (a non-object is
/// `invalid_type`); `parse_lenient` defaults invalid fields, keeps valid ones,
/// turns a `catch` failure into its valid fallback, and lists valid / error
/// fields; the result displays, serializes to JSON (text and file),
/// splits into value and fields; files parse like text and a missing file is
/// `io_error`; renamed fields are reported and looked up by their JSON key.
#[test]
fn lenient_profile_import() {
    let fields = Profile::validate_fields(&profile("X", "bad", "ok_nick")).unwrap();
    let state: Vec<(&str, bool)> = fields
        .iter()
        .map(|f| (f.name.as_str(), f.is_ok()))
        .collect();
    assert_eq!(
        state,
        [
            ("name", false),
            ("email", false),
            ("age", true),
            ("role", true),
            ("nick", true),
            ("inner", true)
        ]
    );
    assert!(fields[1].is_err());
    let err = Profile::validate_fields(&json!("not an object")).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_type");

    let r = Profile::parse_lenient(&profile("Alex", "a@b.com", "alexdev")).unwrap();
    assert!(r.is_valid() && !r.has_errors());
    assert_eq!((r.valid_count(), r.error_count()), (6, 0));

    let r = Profile::parse_lenient(&profile("X", "bad", "!")).unwrap();
    assert_eq!(r.error_count(), 2);
    assert_eq!((r.value.name.as_str(), r.value.email.as_str()), ("", ""));
    assert_eq!(r.value.age, Some(25));
    assert_eq!(r.value.role, "admin");
    assert_eq!(r.value.nick, "anon");
    assert!(r.field("nick").unwrap().is_ok());
    assert_eq!(r.value.inner.city, "NY");
    let names: Vec<&str> = r.error_fields().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["name", "email"]);
    assert_eq!(r.valid_fields().len(), 4);
    assert!(Profile::parse_lenient(&json!(42)).is_err());

    let r = Profile::parse_lenient(&profile("X", "a@b.com", "dev")).unwrap();
    let text = r.to_string();
    assert!(text.contains("valid") && text.contains("error"), "{text}");
    assert_eq!(r.to_json_value().unwrap()["name"], "");
    assert!(r.to_json_string().unwrap().contains("a@b.com"));
    let dir = tempdir();
    let path = dir.path().join("out.json");
    r.save_to_file(&path).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["email"], "a@b.com");
    let (user, fields) = r.into_parts();
    assert_eq!((user.name.as_str(), fields.len()), ("", 6));
    let r = Profile::parse_lenient(&profile("Alex", "a@b.com", "dev")).unwrap();
    assert_eq!(r.into_value().name, "Alex");

    let good = dir.path().join("good.json");
    std::fs::write(&good, profile("Alex", "a@b.com", "dev").to_string()).unwrap();
    assert_eq!(Profile::parse(good.as_path()).unwrap().name, "Alex");
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, profile("X", "bad", "!").to_string()).unwrap();
    let r = Profile::parse_lenient(bad.as_path()).unwrap();
    assert_eq!((r.value.name.as_str(), r.value.nick.as_str()), ("", "anon"));
    let err = Profile::parse(dir.path().join("missing.json").as_path()).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "io_error");

    let fields = RenamedProfile::validate_fields(r#"{"userName": "Al", "age": 25}"#).unwrap();
    assert_eq!(fields[0].name, "userName");
    assert!(fields[0].is_ok());
    assert_eq!(
        (&fields[0].input, &fields[1].input),
        (&json!("Al"), &json!(25))
    );
    let r = RenamedProfile::parse_lenient(r#"{"userName": "X", "age": 25}"#).unwrap();
    assert!(r.field("userName").unwrap().is_err());
    assert_eq!(r.field("userName").unwrap().input, json!("X"));
    assert_eq!(r.field("age").unwrap().input, json!(25));
    assert!(r.field("user_name").is_none());
}

vld::schema! {
    #[derive(Debug)]
    struct Query {
        q: String => vld::string(),
        age: i64 => vld::number().int(),
        zip: String => vld::string(),
        tags: Vec<String> => vld::array(vld::string()),
        ids: Vec<String> => vld::array(vld::string()),
        flag: String => vld::string(),
    }
}

/// Given a query-string type, and `schema!` / `schema_validated!` types
/// carrying `#[into_params(...)]` (as the utoipa integration writes them)
/// When values coerced from text are parsed with `parse_coerced`, and the
/// validated type is parsed strictly and leniently
/// Then each value follows its field's schema (numbers and booleans become
/// strings where the field is a string, `"01234"` stays a string, a scalar
/// becomes a one-element array) while a real type error is still one
/// `invalid_type`; the `into_params` attribute is accepted, and its
/// `parameter_in` is reported (OpenAPI) wherever it appears in the attribute.
#[test]
fn query_parameters() {
    let v = json!({"q": 123, "age": 30, "zip": "01234", "tags": "a", "ids": 7, "flag": true});
    let q: Query = vld::schema::parse_coerced(&v).unwrap();
    assert_eq!(q.q, "123");
    assert_eq!(q.age, 30);
    assert_eq!(q.zip, "01234");
    assert_eq!(q.tags, ["a"]);
    assert_eq!(q.ids, ["7"]);
    assert_eq!(q.flag, "true");
    let v = json!({"q": 1, "age": "abc", "zip": "1", "tags": [], "ids": [], "flag": "x"});
    let err = vld::schema::parse_coerced::<Query>(&v).unwrap_err();
    assert_eq!(issues(&err), iss(&[(".age", "invalid_type")]));
    // A number becomes exactly the text it was written as, beyond f64 precision too.
    for (n, text) in [
        ("9007199254740993", "9007199254740993"),
        ("-9007199254740993", "-9007199254740993"),
        ("18446744073709551615", "18446744073709551615"),
        ("1e20", "100000000000000000000"),
        ("-2.5", "-2.5"),
    ] {
        let v: serde_json::Value = serde_json::from_str(&format!(
            r#"{{"q": {n}, "age": 1, "zip": "1", "tags": [], "ids": [], "flag": "x"}}"#
        ))
        .unwrap();
        let q: Query = vld::schema::parse_coerced(&v).unwrap();
        assert_eq!(q.q, text, "{n}");
    }

    vld::schema! {
        #[derive(Debug)]
        #[into_params(parameter_in = Query)]
        struct Probe {
            x: i64 => vld::number().int(),
        }
    }
    assert_eq!(Probe::parse(r#"{"x": 1}"#).unwrap().x, 1);

    vld::schema_validated! {
        #[derive(Debug, Default, serde::Serialize)]
        #[into_params(parameter_in = Query)]
        pub struct ValidatedRenamed {
            pub full_name: String as "fullName" => vld::string().min(1),
            pub score: f64 => vld::number().min(0.0).max(100.0),
        }
    }
    let json = r#"{"fullName": "Bob", "score": 95.5}"#;
    assert_eq!(ValidatedRenamed::parse(json).unwrap().full_name, "Bob");
    assert!(ValidatedRenamed::parse_lenient(json).unwrap().is_valid());
    let lenient = ValidatedRenamed::parse_lenient(r#"{"fullName": "", "score": 101}"#).unwrap();
    assert_eq!(lenient.error_count(), 2);

    #[cfg(feature = "openapi")]
    {
        use vld::openapi::OpenApiParameterIn;
        vld::schema! {
            #[into_params(parameter_in = Query, rename_all = "camelCase")]
            pub struct QueryWithExtras {
                pub q: String => vld::string(),
            }
        }
        vld::schema! {
            #[into_params(rename_all = "camelCase", parameter_in = Header)]
            pub struct HeaderLast {
                pub q: String => vld::string(),
            }
        }
        assert_eq!(QueryWithExtras::parameter_in(), Some("query"));
        assert_eq!(HeaderLast::parameter_in(), Some("header"));
        // A hand-written `IntoParams` type without a location.
        struct NoLocation;
        impl OpenApiParameterIn for NoLocation {}
        assert_eq!(NoLocation::parameter_in(), None);
    }
}

struct Wire {
    first_name: String,
    count: i64,
}

vld::impl_rules!(Wire {
    first_name as "firstName" => vld::string().min(2),
    count => vld::number().int().min(1),
});

struct Unserializable {
    m: std::collections::HashMap<(i32, i32), i32>,
}

vld::impl_rules!(Unserializable {
    m => vld::any(),
});

/// Given types that are not `schema!` structs, with rules attached by
/// `impl_rules!` (one field renamed for the issue path), one of them holding a
/// map serde cannot serialize to JSON
/// When instances are validated
/// Then good instances pass, bad ones report each field under its rule name
/// (`.firstName`, `.count`), and a field that fails to serialize is a
/// `serialize_error` at its path.
#[test]
fn impl_rules_on_existing_types() {
    let ok = Wire {
        first_name: "Al".into(),
        count: 1,
    };
    assert!(ok.validate().is_ok());
    assert!(ok.is_valid());
    let bad = Wire {
        first_name: "x".into(),
        count: 0,
    };
    assert_eq!(
        issues(&bad.validate().unwrap_err()),
        iss(&[(".firstName", "too_small"), (".count", "too_small")])
    );
    assert!(!bad.is_valid());

    let v = Unserializable {
        m: [((1, 2), 3)].into_iter().collect(),
    };
    assert_eq!(
        issues(&v.validate().unwrap_err()),
        iss(&[(".m", "serialize_error")])
    );
}

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct Config {
        pub host: String => vld::string().with_default("localhost".into()),
        pub port: Option<i64> => vld::number().int().optional(),
        pub tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
    }
}

vld::impl_default!(Config { host, port, tags });

/// Given a config with schema defaults and `impl_default!`
/// When a `Default` value is built and configs are parsed
/// Then `Default` uses the field types' defaults (empty host), while parsing
/// applies the schema defaults (`localhost`) and keeps given values.
#[test]
fn impl_default_gives_type_defaults_parse_gives_schema_defaults() {
    assert_eq!(
        Config::default(),
        Config {
            host: String::new(),
            port: None,
            tags: vec![],
        }
    );
    assert_eq!(Config::parse("{}").unwrap().host, "localhost");
    let parsed = Config::parse(r#"{"host": "example.com", "port": 8080}"#).unwrap();
    assert_eq!(
        (parsed.host.as_str(), parsed.port),
        ("example.com", Some(8080))
    );
}

#[cfg(feature = "openapi")]
mod components {
    use super::*;

    vld::schema! {
        #[derive(Debug, serde::Serialize)]
        pub struct Leaf {
            pub id: String => vld::string().min(1),
        }
    }
    vld::schema! {
        #[derive(Debug)]
        pub struct Left {
            pub leaf: Leaf => vld::nested!(Leaf),
        }
    }
    vld::schema! {
        #[derive(Debug)]
        pub struct Right {
            pub leaves: Option<Vec<Leaf>> => vld::array(vld::nested!(Leaf)).optional(),
        }
    }
    vld::schema! {
        #[derive(Debug)]
        pub struct Root {
            pub left: Left => vld::nested!(Left).message("bad left"),
            pub right: Right => vld::nested!(Right),
        }
    }

    fn leaf_schema() -> serde_json::Value {
        Leaf::json_schema()
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct Holder {
            pub leaf: Leaf => vld::nested_named("Custom.Leaf", Leaf::parse_value, Some(leaf_schema), None),
        }
    }

    /// Given a `Root` nesting `Left` and `Right` that both nest `Leaf` (a
    /// diamond; `Leaf` behind `message`, `optional` and an array), a
    /// `nested_named` field with its own component name, and a dynamic object
    /// nesting `Leaf`
    /// When the OpenAPI document is emitted and nested schemas collected
    /// Then components are transitive and deduplicated (`Leaf`, `Left`,
    /// `Right`, `Root`) and fields `$ref` them; `nested_named` registers
    /// `Custom.Leaf` with the given schema and still parses; the object
    /// collects `Leaf` too.
    #[test]
    fn nested_types_become_openapi_components() {
        let mut names: Vec<&str> = Root::__vld_nested_schemas()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        names.sort();
        assert_eq!(names, ["Leaf", "Left", "Right"]);
        let doc = Root::to_openapi_document();
        let schemas = &doc["components"]["schemas"];
        let mut keys: Vec<&String> = schemas.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["Leaf", "Left", "Right", "Root"]);
        assert_eq!(
            schemas["Root"]["properties"]["left"]["$ref"],
            "#/components/schemas/Left"
        );

        let doc = Holder::to_openapi_document();
        assert_eq!(
            doc["components"]["schemas"]["Holder"]["properties"]["leaf"]["$ref"],
            "#/components/schemas/Custom.Leaf"
        );
        assert_eq!(
            doc["components"]["schemas"]["Custom.Leaf"],
            Leaf::json_schema()
        );
        assert_eq!(
            Holder::parse(r#"{"leaf": {"id": "x"}}"#).unwrap().leaf.id,
            "x"
        );

        use vld::openapi::CollectNestedSchemas;
        let obj = vld::object().field("home", vld::nested!(Leaf).optional());
        let mut out = Vec::new();
        obj.collect_nested_schemas(&mut out);
        assert_eq!(out.iter().map(|(n, _)| *n).collect::<Vec<_>>(), ["Leaf"]);

        // Nested types behind combinators and rules are collected too.
        fn collected(s: &impl CollectNestedSchemas) -> Vec<&'static str> {
            let mut out = Vec::new();
            s.collect_nested_schemas(&mut out);
            out.iter().map(|(n, _)| *n).collect()
        }
        let leaf = || vld::nested!(Leaf);
        let left = || vld::nested!(Left);
        assert_eq!(collected(&vld::union(leaf(), left())), ["Leaf", "Left"]);
        assert_eq!(
            collected(&vld::union3(vld::string(), leaf(), left())),
            ["Leaf", "Left"]
        );
        assert_eq!(
            collected(&vld::intersection(left(), leaf())),
            ["Left", "Leaf"]
        );
        let pets = vld::discriminated_union("kind")
            .variant_str("leaf", leaf())
            .variant_str("other", vld::object());
        assert_eq!(collected(&pets), ["Leaf"]);
        let checked = leaf().rule(vld::rules::from_fn("has_id", |l: &Leaf| !l.id.is_empty()));
        assert_eq!(collected(&checked), ["Leaf"]);
    }
}
