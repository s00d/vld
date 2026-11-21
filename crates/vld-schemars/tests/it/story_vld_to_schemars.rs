//! Stories: vld types (`vld::schema!`, `#[derive(Validate)]`) documented
//! through schemars — `impl_json_schema!` under the default, draft-07 /
//! components-path and OpenAPI 3.0 generators, nested `$ref`s registered as
//! definitions, and the resulting documents validating like vld.

use crate::common::{invalid, valid};
use serde_json::{Value, json};
use vld::openapi::JsonSchema as _;

use vld_schemars::schemars::{self, JsonSchema};
use vld_schemars::*;

vld::schema! {
    #[derive(Debug)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String  => vld::string().min(5).max(10),
    }
}
impl_json_schema!(Address);

vld::schema! {
    #[derive(Debug)]
    pub struct Company {
        pub name: String => vld::string().min(1),
        pub hq: Address => vld::nested!(Address),
    }
}
impl_json_schema!(Company);

vld::schema! {
    #[derive(Debug)]
    pub struct Branch {
        pub home: Option<Address> => vld::nested!(Address)
            .nullable()
            .describe("#/components/schemas/Address"),
        pub tags: (Address, String) => (vld::nested!(Address), vld::string()),
    }
}
impl_json_schema!(Branch);

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}
impl_json_schema!(UserSchema);

vld::schema! {
    #[derive(Debug)]
    pub struct SimpleSchema {
        pub title: String => vld::string().min(1),
    }
}
impl_json_schema!(SimpleSchema, "SimpleItem");

#[derive(Debug, vld::Validate)]
#[allow(dead_code)]
struct DeriveUser {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().gte(0).optional())]
    age: Option<i64>,
}
impl_json_schema!(DeriveUser);

vld::schema! {
    #[derive(Debug)]
    pub struct Oas3 {
        pub pair: (String, u16) => (vld::string(), vld::u16()),
        pub nick: Option<String> => vld::string().nullable(),
        pub score: f64 => vld::number().gt(1.5).lt(9.0),
        pub nothing: () => vld::null(),
        pub home: Option<Address> => vld::nested!(Address).nullable(),
    }
}
impl_json_schema!(Oas3);

fn generate<T: JsonSchema>(
    mut generator: schemars::SchemaGenerator,
) -> (Value, serde_json::Map<String, Value>) {
    let schema = T::json_schema(&mut generator);
    (
        serde_json::to_value(schema).unwrap(),
        generator.definitions().clone(),
    )
}

/// Given `schema!` and derive types, when they implement `schemars::JsonSchema`
/// via `impl_json_schema!`, then the name, id and generated schema carry the
/// vld rules, and the plain `json_schema()` survives the schemars round trip.
#[test]
fn vld_types_become_schemars_types() {
    // Names and ids: the type name or the custom name, ids carry the module.
    assert_eq!(UserSchema::schema_name(), "UserSchema");
    assert_eq!(SimpleSchema::schema_name(), "SimpleItem");
    assert_eq!(DeriveUser::schema_name(), "DeriveUser");
    assert_eq!(
        UserSchema::schema_id(),
        format!("{}::UserSchema", module_path!())
    );
    assert_eq!(
        SimpleSchema::schema_id(),
        format!("{}::SimpleItem", module_path!())
    );

    // The generated schema is vld's own JSON Schema.
    let (user, defs) = generate::<UserSchema>(Default::default());
    assert_eq!(user, UserSchema::json_schema());
    assert!(defs.is_empty(), "a flat type registers nothing: {defs:?}");
    assert_eq!(user["properties"]["name"]["minLength"], 2);
    assert_eq!(user["properties"]["email"]["format"], "email");
    assert_eq!(user["properties"]["age"]["type"], "integer");
    let (derive, _) = generate::<DeriveUser>(Default::default());
    assert_eq!(derive["properties"]["name"]["maxLength"], 50);
    assert_eq!(derive["required"], json!(["name", "email"]));
    let (custom, _) = generate::<SimpleSchema>(Default::default());
    assert_eq!(custom["type"], "object");

    // vld → schemars → JSON is the identity; non-schemas fail closed.
    for js in [
        UserSchema::json_schema(),
        vld::string().email().json_schema(),
        vld::number().int().min(0).json_schema(),
        json!(true),
    ] {
        let s = vld_to_schemars(&js);
        assert_eq!(schemars_to_json(&s), js);
        assert_eq!(schemars_to_json(&vld_schema_to_schemars(&js)), js);
        assert_eq!(schemars_to_json(&try_vld_to_schemars(&js).unwrap()), js);
    }
    for bad in [json!("not a schema"), json!(42), json!(null), json!([1])] {
        assert_eq!(vld_to_schemars(&bad), schemars::Schema::from(false));
        match try_vld_to_schemars(&bad) {
            Err(VldSchemarsError::Deserialization(_)) => {}
            other => panic!("{bad}: {other:?}"),
        }
    }

    // The converted schema validates like vld.
    let s = vld_to_schemars(&UserSchema::json_schema());
    let ok = json!({"name": "Al", "email": "a@b", "age": 3});
    assert!(UserSchema::vld_parse_value(&ok).is_ok());
    validate_with_schemars(&s, &ok).unwrap();
    let bad = json!({"name": "A", "email": "nope", "age": -1});
    assert!(UserSchema::vld_parse_value(&bad).is_err());
    invalid(
        s.as_value(),
        &bad,
        &[
            (".age", "too_small"),
            (".email", "invalid_string"),
            (".name", "too_small"),
        ],
    );
}

/// Given a type with a nested `vld::nested!` field, when schemars generates it
/// with the default, an aide-style (`#/components/schemas/`) and an OpenAPI 3.0
/// generator, then the nested type is a definition and every `$ref` points at
/// the generator's own definitions path; the standalone root schema resolves
/// its refs and validates like vld.
#[test]
fn nested_types_are_registered_where_the_generator_wants_them() {
    let (company, defs) = generate::<Company>(Default::default());
    assert_eq!(company["properties"]["hq"]["$ref"], "#/$defs/Address");
    assert_eq!(defs["Address"], Address::json_schema());

    let aide = schemars::generate::SchemaSettings::draft07()
        .with(|s| s.definitions_path = "#/components/schemas/".into())
        .into_generator();
    let (company, defs) = generate::<Company>(aide);
    assert_eq!(
        company["properties"]["hq"]["$ref"],
        "#/components/schemas/Address"
    );
    assert!(defs.contains_key("Address"));

    let draft07 = schemars::generate::SchemaSettings::draft07().into_generator();
    let (company, _) = generate::<Company>(draft07);
    assert_eq!(company["properties"]["hq"]["$ref"], "#/definitions/Address");

    let oas3 = schemars::generate::SchemaSettings::openapi3().into_generator();
    let (company, _) = generate::<Company>(oas3);
    assert_eq!(
        company["properties"]["hq"]["$ref"],
        "#/components/schemas/Address"
    );

    // Refs inside arrays (`anyOf`, `prefixItems`) are rewritten too; other
    // strings that look like refs are left alone.
    let (branch, defs) = generate::<Branch>(Default::default());
    let home = &branch["properties"]["home"];
    assert_eq!(home["anyOf"][0]["$ref"], "#/$defs/Address", "{home}");
    assert_eq!(home["description"], "#/components/schemas/Address");
    assert_eq!(
        branch["properties"]["tags"]["prefixItems"][0]["$ref"],
        "#/$defs/Address"
    );
    assert!(defs.contains_key("Address"));

    // A definition that is already registered is kept, not replaced.
    let mut generator = schemars::SchemaGenerator::default();
    generator
        .definitions_mut()
        .insert("Address".into(), json!({"title": "mine"}));
    let _ = <Company as JsonSchema>::json_schema(&mut generator);
    assert_eq!(generator.definitions()["Address"], json!({"title": "mine"}));

    // The root document carries its definitions; the validator follows them.
    let root = generate_from_schemars::<Company>();
    let r = root["properties"]["hq"]["$ref"].as_str().unwrap();
    assert!(
        root.pointer(r.strip_prefix('#').unwrap()).is_some(),
        "dangling {r}"
    );
    let ok = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "75001"}});
    let bad = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "1"}});
    assert!(Company::vld_parse_value(&ok).is_ok());
    assert!(Company::vld_parse_value(&bad).is_err());
    valid(&root, &ok);
    invalid(&root, &bad, &[(".hq.zip", "too_small")]);
    assert_eq!(schemars_to_json(&generate_schemars::<Company>()), root);
}

/// Regression for https://github.com/s00d/vld/issues/5: given Schedule → Day
/// → Break, when schemars generates Schedule, then the grandchild is
/// registered too, with refs rewritten inside the child definition.
#[test]
fn grandchildren_are_registered_transitively() {
    vld::schema! {
        #[derive(Debug)]
        pub struct Break {
            pub label: String => vld::string().min(1),
        }
    }
    impl_json_schema!(Break);
    vld::schema! {
        #[derive(Debug)]
        pub struct Day {
            pub breaks: Option<Vec<Break>> => vld::array(vld::nested!(Break)).optional(),
        }
    }
    impl_json_schema!(Day);
    vld::schema! {
        #[derive(Debug)]
        pub struct Schedule {
            pub days: Vec<Day> => vld::array(vld::nested!(Day)),
        }
    }
    impl_json_schema!(Schedule);

    let (schedule, defs) = generate::<Schedule>(Default::default());
    assert_eq!(
        schedule["properties"]["days"]["items"]["$ref"],
        "#/$defs/Day"
    );
    assert_eq!(
        defs["Day"]["properties"]["breaks"]["items"]["$ref"],
        "#/$defs/Break"
    );
    assert_eq!(defs["Break"], Break::json_schema());

    let root = generate_from_schemars::<Schedule>();
    valid(&root, &json!({"days": [{"breaks": [{"label": "x"}]}, {}]}));
    invalid(
        &root,
        &json!({"days": [{"breaks": [{"label": ""}]}]}),
        &[(".days[0].breaks[0].label", "too_small")],
    );
}

/// Given a type with a tuple, nullable fields, exclusive bounds, a `null`
/// field and a nullable ref, when an OpenAPI 3.0 generator documents it, then
/// the schema is OpenAPI 3.0 (no `prefixItems`, `nullable` instead of `null`,
/// boolean exclusive bounds) — the same as `to_openapi3`, which is idempotent.
#[test]
fn openapi3_generators_get_openapi3_schemas() {
    let generator = schemars::generate::SchemaSettings::openapi3().into_generator();
    let root = serde_json::to_value(generator.into_root_schema_for::<Oas3>()).unwrap();
    let p = &root["properties"];

    let pair = &p["pair"];
    assert_eq!(pair["type"], "array");
    assert!(pair.get("prefixItems").is_none(), "{pair}");
    assert!(pair.get("additionalItems").is_none(), "{pair}");
    assert_eq!(pair["items"]["anyOf"][0]["type"], "string", "{pair}");
    assert_eq!(pair["items"]["anyOf"][1]["type"], "integer", "{pair}");
    assert_eq!(
        (pair["minItems"].clone(), pair["maxItems"].clone()),
        (json!(2), json!(2))
    );

    let nick = &p["nick"];
    assert_eq!(nick["type"], "string", "{nick}");
    assert_eq!(nick["nullable"], true, "{nick}");
    assert!(nick.get("enum").is_none(), "{nick}");
    assert!(nick.get("anyOf").is_none(), "{nick}");

    let score = &p["score"];
    assert_eq!(score["minimum"], 1.5, "{score}");
    assert_eq!(score["exclusiveMinimum"], true, "{score}");
    assert_eq!(score["maximum"], 9.0, "{score}");
    assert_eq!(score["exclusiveMaximum"], true, "{score}");

    assert_eq!(
        p["nothing"],
        json!({"type": "string", "nullable": true, "enum": [null]})
    );

    let home = &p["home"];
    assert_eq!(home["nullable"], true, "{home}");
    assert_eq!(
        home["allOf"],
        json!([{"$ref": "#/components/schemas/Address"}]),
        "{home}"
    );

    // `to_openapi3` by hand gives the same, and twice is once.
    let mut by_hand = Oas3::json_schema();
    to_openapi3(&mut by_hand);
    assert_eq!(by_hand["properties"]["nick"], *nick);
    assert_eq!(by_hand["properties"]["score"], *score);
    let mut again = by_hand.clone();
    to_openapi3(&mut again);
    assert_eq!(again, by_hand);
}

/// Given hand-written 2020-12 schemas, when `to_openapi3` rewrites them, then
/// each keyword maps to its 3.0 form.
#[test]
fn to_openapi3_rewrites_each_keyword() {
    let cases = [
        (
            json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10}),
            json!({"type": "number", "minimum": 0, "exclusiveMinimum": true, "maximum": 10, "exclusiveMaximum": true}),
        ),
        // A stricter inclusive bound wins; a looser one is replaced.
        (
            json!({"type": "integer", "minimum": 5, "exclusiveMinimum": 0, "maximum": 20, "exclusiveMaximum": 10}),
            json!({"type": "integer", "minimum": 5, "maximum": 10, "exclusiveMaximum": true}),
        ),
        (
            json!({"maximum": 1, "exclusiveMaximum": 10, "minimum": 0, "exclusiveMinimum": 0}),
            json!({"maximum": 1, "minimum": 0, "exclusiveMinimum": true}),
        ),
        (
            json!({"exclusiveMinimum": true}),
            json!({"exclusiveMinimum": true}),
        ),
        (
            json!({"type": ["string", "null"]}),
            json!({"type": "string", "nullable": true}),
        ),
        (
            json!({"type": ["string", "integer", "null"]}),
            json!({"type": ["string", "integer"], "nullable": true}),
        ),
        (
            json!({"type": ["string", "integer"]}),
            json!({"type": ["string", "integer"]}),
        ),
        (
            json!({"type": ["null"]}),
            json!({"type": "string", "nullable": true, "enum": [null]}),
        ),
        (
            json!({"oneOf": [{"type": "string"}, {"type": "integer"}, {"const": null}]}),
            json!({"oneOf": [{"type": "string"}, {"type": "integer"}], "nullable": true}),
        ),
        (
            json!({"anyOf": [{"enum": [null]}, {"type": "string", "description": "inner"}], "description": "outer"}),
            json!({"type": "string", "nullable": true, "description": "outer"}),
        ),
        (
            json!({"anyOf": [{"type": "null"}, true]}),
            json!({"allOf": [true], "nullable": true}),
        ),
        (
            json!({"anyOf": [{"type": "null"}]}),
            json!({"type": "string", "nullable": true, "enum": [null]}),
        ),
        (
            json!({"const": null}),
            json!({"type": "string", "nullable": true, "enum": [null]}),
        ),
        (
            json!({"type": "array", "prefixItems": [{"type": "string"}]}),
            json!({"type": "array", "items": {"type": "string"}}),
        ),
        (
            json!([{"type": "null"}, {"type": "array", "prefixItems": []}]),
            json!([{"type": "string", "nullable": true, "enum": [null]}, {"type": "array", "items": {"anyOf": []}}]),
        ),
        (json!("x"), json!("x")),
        // A lone remaining `$ref` cannot take siblings in 3.0: `allOf`.
        (
            json!({"anyOf": [{"$ref": "#/components/schemas/A"}, {"type": "null"}], "description": "a"}),
            json!({"allOf": [{"$ref": "#/components/schemas/A"}], "nullable": true, "description": "a"}),
        ),
    ];
    for (before, after) in cases {
        let mut s = before.clone();
        to_openapi3(&mut s);
        assert_eq!(s, after, "{before}");
        to_openapi3(&mut s);
        assert_eq!(s, after, "second pass on {before}");
    }

    // From real vld schemas.
    let mut n = vld::number().gt(1.5).json_schema();
    to_openapi3(&mut n);
    assert_eq!(
        (n["minimum"].clone(), n["exclusiveMinimum"].clone()),
        (json!(1.5), json!(true))
    );
    let mut null = vld::null().json_schema();
    to_openapi3(&mut null);
    assert_eq!(
        null,
        json!({"type": "string", "nullable": true, "enum": [null]})
    );
}

/// Given a vld `with_default` / `describe` / `nullish` matrix, when schemars
/// documents both a `schema!` and a derive type, then the required sets match
/// vld's (defaults, optional and nullish are not required; nullable is).
#[test]
fn required_matrix_matches_vld() {
    vld::schema! {
        #[derive(Debug)]
        pub struct CreateTicket {
            pub title: String => vld::string().min(3).max(120).describe("ticket title"),
            pub body: String => vld::string().min(1),
            pub priority: String => vld::string().with_default("normal".into()),
            pub labels: Option<Vec<String>> => vld::array(vld::string().min(1)).optional(),
            pub due_at: Option<String> => vld::string().nullish().describe("ISO date"),
            pub reporter: Option<String> => vld::string().email().nullable(),
        }
    }
    impl_json_schema!(CreateTicket);
    let (doc, _) = generate::<CreateTicket>(Default::default());
    let req: Vec<_> = list_properties(&doc)
        .into_iter()
        .filter(|p| p.required)
        .map(|p| p.name)
        .collect();
    assert_eq!(req, ["body", "reporter", "title"]);
    assert_eq!(doc["properties"]["title"]["description"], "ticket title");
    assert_eq!(doc["properties"]["priority"]["default"], "normal");
    // Missing defaulted / optional / nullish fields parse in vld and validate.
    let minimal = json!({"title": "abc", "body": "x", "reporter": null});
    assert!(CreateTicket::vld_parse_value(&minimal).is_ok());
    valid(&doc, &minimal);
    let missing = json!({"title": "abc", "body": "x"});
    assert!(CreateTicket::vld_parse_value(&missing).is_err());
    invalid(&doc, &missing, &[(".reporter", "missing_field")]);
    // The same verdicts as vld on a few bodies.
    for body in [
        json!({"title": "ab", "body": "x", "reporter": null}),
        json!({"title": "abc", "body": "", "reporter": "a@b"}),
        json!({"title": "abc", "body": "x", "reporter": "nope", "labels": [""]}),
        json!({"title": "abc", "body": "x", "reporter": "a@b", "labels": ["l"], "due_at": null}),
    ] {
        assert_eq!(
            CreateTicket::vld_parse_value(&body).is_ok(),
            validate_with_schema(&doc, &body).is_ok(),
            "{body}"
        );
    }
}
