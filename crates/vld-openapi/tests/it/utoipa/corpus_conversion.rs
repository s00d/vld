//! Corpus: `json_schema_to_schema` keyword by keyword. Each row is a JSON
//! Schema (hand written or emitted by vld) and the values expected at JSON
//! pointers of the serialized utoipa schema (`None` = the key is absent).
//! Pointers rather than whole documents: utoipa 5 and 6 may serialize extra
//! defaults differently, the mapped keywords must not differ.

use super::bridge::json_schema_to_schema;
use super::bridge::utoipa::Number;
use super::bridge::utoipa::openapi::{RefOr, schema::Schema};
use serde_json::{Value, json};
use vld::openapi::JsonSchema;
use vld::schema::VldSchema as _;

type Row = (Value, Vec<(&'static str, Option<Value>)>);

fn conv(v: &Value) -> Value {
    serde_json::to_value(json_schema_to_schema(v)).unwrap()
}

fn rows() -> Vec<Row> {
    let s = |v: Value| Some(v);
    vec![
        // primitives and type arrays; a missing type is "any value"
        (
            json!({"type": "string"}),
            vec![("/type", s(json!("string")))],
        ),
        (
            json!({"type": "boolean"}),
            vec![("/type", s(json!("boolean")))],
        ),
        (json!({"type": "null"}), vec![("/type", s(json!("null")))]),
        (
            json!({"type": "object"}),
            vec![("/type", s(json!("object")))],
        ),
        (json!({}), vec![("/type", None)]),
        (json!({"type": "weird"}), vec![("/type", None)]),
        (json!({"type": 3}), vec![("/type", None)]),
        (json!(true), vec![("/type", None), ("/not", None)]),
        (json!("not a schema"), vec![("/type", None)]),
        (
            json!({"description": "anything"}),
            vec![("/type", None), ("/description", s(json!("anything")))],
        ),
        (
            json!({"type": ["string", "null"], "minLength": 2}),
            vec![
                ("/type", s(json!(["string", "null"]))),
                ("/minLength", s(json!(2))),
            ],
        ),
        (
            json!({"type": ["integer", "bogus"]}),
            vec![("/type", s(json!("integer")))],
        ),
        (
            json!({"type": ["array", "null"], "items": {"type": "integer"}}),
            vec![
                ("/type", s(json!(["array", "null"]))),
                ("/items/type", s(json!("integer"))),
            ],
        ),
        // strings
        (
            json!({"type": "string", "minLength": 2, "maxLength": 50, "format": "email", "pattern": "^[a-z]+$"}),
            vec![
                ("/minLength", s(json!(2))),
                ("/maxLength", s(json!(50))),
                ("/format", s(json!("email"))),
                ("/pattern", s(json!("^[a-z]+$"))),
            ],
        ),
        (
            json!({"type": "string", "description": "User name", "default": "anonymous", "title": "Name", "example": "bob"}),
            vec![
                ("/description", s(json!("User name"))),
                ("/default", s(json!("anonymous"))),
                ("/title", s(json!("Name"))),
                ("/example", s(json!("bob"))),
            ],
        ),
        (
            json!({"type": "string", "enum": ["a", "b", "c"]}),
            vec![("/enum", s(json!(["a", "b", "c"])))],
        ),
        (
            json!({"type": "string", "const": "x"}),
            vec![("/enum", s(json!(["x"])))],
        ),
        (
            json!({"type": "string", "enum": ["a"], "const": "x"}),
            vec![("/enum", s(json!(["a"])))],
        ),
        // numbers: integers stay integers, u64 bounds stay exact
        (
            json!({"type": "integer", "minimum": 0, "maximum": 100}),
            vec![
                ("/type", s(json!("integer"))),
                ("/minimum", s(json!(0))),
                ("/maximum", s(json!(100))),
            ],
        ),
        (
            json!({"type": "number", "minimum": 0.5, "maximum": 99.9}),
            vec![("/minimum", s(json!(0.5))), ("/maximum", s(json!(99.9)))],
        ),
        (
            json!({"type": "number", "minimum": 2.0, "maximum": -3.0}),
            vec![("/minimum", s(json!(2))), ("/maximum", s(json!(-3)))],
        ),
        (
            json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 100}),
            vec![
                ("/exclusiveMinimum", s(json!(0))),
                ("/exclusiveMaximum", s(json!(100))),
            ],
        ),
        (
            json!({"type": "integer", "multipleOf": 5}),
            vec![("/multipleOf", s(json!(5)))],
        ),
        (
            json!({"type": "number", "multipleOf": 0.25}),
            vec![("/multipleOf", s(json!(0.25)))],
        ),
        (
            json!({"type": "number", "maximum": 1e300}),
            vec![("/maximum", s(json!(1e300)))],
        ),
        (
            json!({"type": "number", "minimum": "5"}),
            vec![("/minimum", None)],
        ),
        (
            vld::u64().json_schema(),
            vec![("/maximum", s(json!(u64::MAX)))],
        ),
        (
            vld::i64().json_schema(),
            vec![
                ("/minimum", s(json!(i64::MIN))),
                ("/maximum", s(json!(i64::MAX))),
            ],
        ),
        // arrays
        (
            json!({"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 10}),
            vec![
                ("/type", s(json!("array"))),
                ("/items/type", s(json!("string"))),
                ("/minItems", s(json!(1))),
                ("/maxItems", s(json!(10))),
            ],
        ),
        (
            json!({"type": "array"}),
            vec![("/items/type", None), ("/uniqueItems", None)],
        ),
        (
            json!({"type": "array", "items": {"type": "string"}, "uniqueItems": true, "description": "tags", "default": [], "title": "Tags"}),
            vec![
                ("/uniqueItems", s(json!(true))),
                ("/description", s(json!("tags"))),
                ("/default", s(json!([]))),
                ("/title", s(json!("Tags"))),
            ],
        ),
        (
            (vld::string(), vld::u16()).json_schema(),
            vec![
                ("/prefixItems/0/type", s(json!("string"))),
                ("/prefixItems/1/type", s(json!("integer"))),
                ("/items", s(json!(false))),
                ("/minItems", s(json!(2))),
                ("/maxItems", s(json!(2))),
            ],
        ),
        (
            json!({"type": "array", "prefixItems": [{"$ref": "#/components/schemas/A"}], "items": false}),
            vec![(
                "/prefixItems/0/allOf/0/$ref",
                s(json!("#/components/schemas/A")),
            )],
        ),
        // objects
        (
            json!({"type": "object", "required": ["name", "email", 3], "properties": {
            "name": {"type": "string", "minLength": 1}, "email": {"type": "string", "format": "email"}}}),
            vec![
                ("/type", s(json!("object"))),
                ("/properties/name/minLength", s(json!(1))),
                ("/properties/email/format", s(json!("email"))),
                ("/required", s(json!(["name", "email"]))),
            ],
        ),
        (
            json!({"type": "object", "additionalProperties": {"type": "integer"}, "minProperties": 1, "maxProperties": 3}),
            vec![
                ("/additionalProperties/type", s(json!("integer"))),
                ("/minProperties", s(json!(1))),
                ("/maxProperties", s(json!(3))),
            ],
        ),
        (
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
            vec![("/additionalProperties", s(json!(false)))],
        ),
        (
            json!({"type": "object", "additionalProperties": true}),
            vec![("/additionalProperties", s(json!(true)))],
        ),
        (
            json!({"type": "object", "additionalProperties": 1}),
            vec![("/additionalProperties", None)],
        ),
        (
            vld::record(vld::number().int()).json_schema(),
            vec![("/additionalProperties/type", s(json!("integer")))],
        ),
        // composites keep their annotations
        (
            json!({"oneOf": [{"type": "string"}, {"type": "integer"}]}),
            vec![
                ("/oneOf/0/type", s(json!("string"))),
                ("/oneOf/1/type", s(json!("integer"))),
            ],
        ),
        (
            json!({"allOf": [{"type": "string", "minLength": 3}, {"type": "string", "format": "email"}]}),
            vec![
                ("/allOf/0/minLength", s(json!(3))),
                ("/allOf/1/format", s(json!("email"))),
                ("/allOf/2", None),
            ],
        ),
        (
            json!({"oneOf": [{"type": "string"}, {"type": "null"}], "description": "d", "default": "x", "title": "T"}),
            vec![
                ("/oneOf/1/type", s(json!("null"))),
                ("/description", s(json!("d"))),
                ("/default", s(json!("x"))),
                ("/title", s(json!("T"))),
            ],
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}], "description": "d", "default": "x", "example": "e"}),
            vec![
                ("/anyOf/1/type", s(json!("null"))),
                ("/description", s(json!("d"))),
                ("/default", s(json!("x"))),
                ("/example", s(json!("e"))),
            ],
        ),
        (
            json!({"allOf": [{"type": "string"}, {"type": "null"}], "description": "d", "default": "x", "title": "T"}),
            vec![
                ("/allOf/1/type", s(json!("null"))),
                ("/description", s(json!("d"))),
                ("/default", s(json!("x"))),
                ("/title", s(json!("T"))),
            ],
        ),
        // vld's extra patterns / multipleOf sit in `allOf` next to `type`
        (
            vld::string()
                .min(3)
                .starts_with("a")
                .ends_with("z")
                .json_schema(),
            vec![
                ("/allOf/0/type", s(json!("string"))),
                ("/allOf/0/minLength", s(json!(3))),
                ("/allOf/0/pattern", s(json!("^a"))),
                ("/allOf/1/pattern", s(json!("z$"))),
            ],
        ),
        (
            vld::number().multiple_of(3.).multiple_of(5.).json_schema(),
            vec![
                ("/allOf/0/type", s(json!("number"))),
                ("/allOf/0/multipleOf", s(json!(3))),
                ("/allOf/1/multipleOf", s(json!(5))),
            ],
        ),
        (
            json!({"allOf": [{"type": "string"}, {"minLength": 1}], "description": "d", "x-k": 1}),
            vec![
                ("/allOf/0/type", s(json!("string"))),
                ("/allOf/1/minLength", s(json!(1))),
                ("/allOf/2", None),
                ("/description", s(json!("d"))),
                ("/x-k", s(json!(1))),
            ],
        ),
        // siblings of oneOf / anyOf go into every non-null branch
        (
            json!({"type": "string", "anyOf": [{"format": "date-time"}, {"pattern": "^a$"}, {"type": "null"}, true], "minLength": 2, "description": "when", "x-past": true}),
            vec![
                ("/anyOf/0/type", s(json!("string"))),
                ("/anyOf/0/format", s(json!("date-time"))),
                ("/anyOf/1/type", s(json!("string"))),
                ("/anyOf/1/pattern", s(json!("^a$"))),
                ("/anyOf/2/type", s(json!("null"))),
                ("/anyOf/2/minLength", None),
                ("/anyOf/1/minLength", s(json!(2))),
                ("/anyOf/3/type", None),
                ("/description", s(json!("when"))),
                ("/x-past", s(json!(true))),
                ("/type", None),
            ],
        ),
        (
            json!({"minLength": 1, "oneOf": [{"type": "string", "minLength": 5}]}),
            vec![("/oneOf/0/minLength", s(json!(5)))],
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            vec![("/anyOf/1/type", s(json!("null")))],
        ),
        (
            json!({"anyOf": [{"$ref": "#/components/schemas/Address"}, {"type": "null"}]}),
            vec![
                ("/type", None),
                ("/anyOf/0/$ref", s(json!("#/components/schemas/Address"))),
            ],
        ),
        (
            json!({"type": "object", "properties": {"addr": {"anyOf": [{"$ref": "#/components/schemas/Address"}, {"type": "null"}]}}}),
            vec![(
                "/properties/addr/anyOf/0/$ref",
                s(json!("#/components/schemas/Address")),
            )],
        ),
        // $ref: description stays on the ref; title / default / example wrap it
        (
            json!({"$ref": "#/components/schemas/A", "description": "addr"}),
            vec![
                ("/$ref", s(json!("#/components/schemas/A"))),
                ("/description", s(json!("addr"))),
            ],
        ),
        (
            json!({"$ref": "#/components/schemas/A", "description": "addr", "default": {"x": 1}, "title": "T"}),
            vec![
                ("/allOf/0/$ref", s(json!("#/components/schemas/A"))),
                ("/description", s(json!("addr"))),
                ("/default", s(json!({"x": 1}))),
                ("/title", s(json!("T"))),
            ],
        ),
        (
            json!({"$ref": "#/components/schemas/A", "example": 1}),
            vec![
                ("/allOf/0/$ref", s(json!("#/components/schemas/A"))),
                ("/example", s(json!(1))),
            ],
        ),
        // never, extensions and keywords utoipa has no field for
        (
            vld::never().json_schema(),
            vec![("", s(json!({"not": {}})))],
        ),
        (json!(false), vec![("", s(json!({"not": {}})))]),
        (
            vld::array(vld::number())
                .contains(3)
                .min_contains(2)
                .max_contains(3)
                .json_schema(),
            vec![
                ("/contains", s(json!({"const": 3}))),
                ("/minContains", s(json!(2))),
                ("/maxContains", s(json!(3))),
            ],
        ),
        (
            vld::set(vld::boolean()).max_size(2).json_schema(),
            vec![("/x-maxUniqueItems", s(json!(2)))],
        ),
        (
            vld::record_with_keys(vld::string().min(2), vld::number()).json_schema(),
            vec![("/propertyNames/minLength", s(json!(2)))],
        ),
        (
            json!({"type": "string", "format": "date", "formatMinimum": "2020-01-01", "formatMaximum": "2020-12-31", "x-past": true}),
            vec![
                ("/formatMinimum", s(json!("2020-01-01"))),
                ("/formatMaximum", s(json!("2020-12-31"))),
                ("/x-past", s(json!(true))),
            ],
        ),
        (
            json!({"not": {"type": "string"}}),
            vec![("/not/type", s(json!("string")))],
        ),
        // vld round trips
        (
            vld::string().min(3).max(100).email().json_schema(),
            vec![
                ("/type", s(json!("string"))),
                ("/minLength", s(json!(3))),
                ("/maxLength", s(json!(100))),
                ("/format", s(json!("email"))),
            ],
        ),
        (
            vld::number().min(0.0).max(100.0).json_schema(),
            vec![("/type", s(json!("number")))],
        ),
        (
            vld::array(vld::string().non_empty())
                .min_len(1)
                .json_schema(),
            vec![
                ("/type", s(json!("array"))),
                ("/items/type", s(json!("string"))),
                ("/minItems", s(json!(1))),
            ],
        ),
        (
            vld::string().email().optional().json_schema(),
            vec![("/format", s(json!("email")))],
        ),
        (
            vld::string().email().nullish().json_schema(),
            vec![
                ("/anyOf/0/format", s(json!("email"))),
                ("/anyOf/1/type", s(json!("null"))),
            ],
        ),
    ]
}

/// Given each row's JSON Schema, when it is converted to a utoipa schema and
/// serialized, then every listed pointer holds the expected value.
#[test]
fn every_keyword_maps_to_utoipa() {
    let mut failures = Vec::new();
    for (input, expect) in rows() {
        let out = conv(&input);
        for (ptr, want) in expect {
            let got = out.pointer(ptr).cloned();
            if got != want {
                failures.push(format!(
                    "{input} {ptr}: got {got:?}, want {want:?} in {out}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // A bare `$ref` stays a utoipa `Ref`.
    match json_schema_to_schema(&json!({"$ref": "#/components/schemas/User"})) {
        RefOr::Ref(r) => assert_eq!(r.ref_location, "#/components/schemas/User"),
        RefOr::T(_) => panic!("expected a Ref"),
    }
    // u64 bounds stay integers, not floats; whole floats become integers;
    // integers beyond 2^53 stay exact.
    assert!(conv(&vld::u64().json_schema())["maximum"].is_u64());
    let whole = conv(&json!({"type": "number", "minimum": 2.0, "maximum": -3.0}));
    assert!(
        whole["minimum"].is_i64() && whole["maximum"].is_i64(),
        "{whole}"
    );
    let big = conv(&json!({"type": "integer", "minimum": 9007199254740993i64}));
    assert_eq!(big["minimum"].as_i64(), Some(9007199254740993), "{big}");
    // In the utoipa struct too: whole floats are `Int`, 2^63 stays `Float`.
    let minimum = |v: Value| match json_schema_to_schema(&v) {
        RefOr::T(Schema::Object(o)) => o.minimum,
        _ => None,
    };
    assert!(minimum(json!({"minimum": 2.0})) == Some(Number::Int(2)));
    assert!(minimum(json!({"minimum": -3.0})) == Some(Number::Int(-3)));
    assert!(
        minimum(json!({"minimum": 9.223372036854776e18}))
            == Some(Number::Float(9.223372036854776e18))
    );
    assert!(minimum(json!({"minimum": 7})) == Some(Number::Int(7)));
    let frac = conv(&json!({"type": "number", "minimum": 0.5}));
    assert!(frac["minimum"].is_f64(), "{frac}");
}
