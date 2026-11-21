//! Stories: comparing two versions of a JSON Schema with `vld::diff::diff_schemas` (feature
//! `diff`) to tell breaking from non-breaking changes.

use serde_json::{Value, json};
use vld::diff::{ChangeKind, SchemaDiff, diff_schemas};
use vld::schema::VldSchema;

/// `(path, kind)` of every change.
fn summary(d: &SchemaDiff) -> Vec<(String, ChangeKind)> {
    d.changes.iter().map(|c| (c.path.clone(), c.kind)).collect()
}

/// An object with one property `a`, required or not.
fn obj(prop: Value, required: bool) -> Value {
    let mut v = json!({"type": "object", "properties": {"a": prop}});
    if required {
        v["required"] = json!(["a"]);
    }
    v
}

/// Given a schema compared with itself, then pairs that differ in one keyword
/// When each pair is diffed
/// Then the identical pair has no changes; every other pair has exactly one change, breaking when
/// the new schema accepts less (tighter bound, narrower type or enum, new format / const /
/// uniqueness / contains / past, other `$ref`, dropped `null`) and non-breaking when it accepts
/// more; breaking + non-breaking lists partition the changes.
#[test]
fn single_keyword_changes_are_classified() {
    let s = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    let d = diff_schemas(&s, &s);
    assert!(d.changes.is_empty());
    assert!(!d.has_breaking());

    use ChangeKind::{Breaking as B, NonBreaking as N};
    let cases = [
        (json!({"type": "string"}), json!({"type": "number"}), B),
        (
            json!({"type": "number", "minimum": 0}),
            json!({"type": "number", "minimum": 5}),
            B,
        ),
        (
            json!({"type": "number", "minimum": 5}),
            json!({"type": "number", "minimum": 0}),
            N,
        ),
        (
            json!({"type": "number", "maximum": 5}),
            json!({"type": "number", "maximum": 3}),
            B,
        ),
        (
            json!({"type": "string", "maxLength": 10}),
            json!({"type": "string", "maxLength": 20}),
            N,
        ),
        (
            json!({"type": "string"}),
            json!({"type": "string", "format": "email"}),
            B,
        ),
        (
            json!({"type": "string", "format": "email"}),
            json!({"type": "string"}),
            N,
        ),
        (
            json!({"enum": ["a", "b", "c"]}),
            json!({"enum": ["a", "b"]}),
            B,
        ),
        (
            json!({"enum": ["a", "b"]}),
            json!({"enum": ["a", "b", "c"]}),
            N,
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            json!({"type": "string"}),
            B,
        ),
        (
            json!({"$ref": "#/components/schemas/A"}),
            json!({"$ref": "#/components/schemas/B"}),
            B,
        ),
        (json!({"const": 1}), json!({"const": 2}), B),
        (json!({}), json!({"const": 2}), B),
        (json!({"const": 1}), json!({}), N),
        (json!({"multipleOf": 2}), json!({"multipleOf": 4}), B),
        (json!({"multipleOf": 4}), json!({"multipleOf": 2}), N),
        (json!({"multipleOf": 2}), json!({"multipleOf": 3}), B),
        (
            json!({"prefixItems": [{}]}),
            json!({"prefixItems": [{}, {}]}),
            B,
        ),
        (
            json!({"prefixItems": [{}, {}]}),
            json!({"prefixItems": [{}]}),
            N,
        ),
        (
            json!({"prefixItems": [{"type": "string"}]}),
            json!({"prefixItems": [{"type": "number"}]}),
            B,
        ),
        (json!({}), json!({"uniqueItems": true}), B),
        (json!({"uniqueItems": true}), json!({}), N),
        (json!({"minProperties": 1}), json!({"minProperties": 2}), B),
        (json!({"maxProperties": 2}), json!({"maxProperties": 3}), N),
        (json!({}), json!({"propertyNames": {"pattern": "^a"}}), B),
        (
            json!({"propertyNames": {"pattern": "^a"}}),
            json!({"propertyNames": {}}),
            N,
        ),
        (json!({}), json!({"contains": {"const": 1}}), B),
        (json!({"minContains": 1}), json!({"minContains": 2}), B),
        (json!({"maxContains": 3}), json!({"maxContains": 2}), B),
        (
            json!({"formatMinimum": "2020-01-01"}),
            json!({"formatMinimum": "2021-01-01"}),
            B,
        ),
        (
            json!({"formatMaximum": "2020-01-01"}),
            json!({"formatMaximum": "2021-01-01"}),
            N,
        ),
        (
            json!({"x-minDecimal": "1.5"}),
            json!({"x-minDecimal": "2"}),
            B,
        ),
        (
            json!({"x-maxUniqueItems": 5}),
            json!({"x-maxUniqueItems": 6}),
            N,
        ),
        (json!({}), json!({"x-past": true}), B),
    ];
    for (old, new, want) in cases {
        let d = diff_schemas(&old, &new);
        assert_eq!(d.changes.len(), 1, "{old} -> {new}: {d}");
        assert_eq!(d.changes[0].kind, want, "{old} -> {new}");
        assert_eq!(d.has_breaking(), want == B, "{old} -> {new}");
        assert_eq!(
            d.breaking_changes().len() + d.non_breaking_changes().len(),
            d.changes.len()
        );
    }

    // Exact reports: path, kind and description of every change, for keywords appearing,
    // disappearing, being rewritten to an equal value, and combined with other changes.
    let exact = |old: Value, new: Value, want: &[(&str, ChangeKind, &str)]| {
        let got: Vec<(String, ChangeKind, String)> = diff_schemas(&old, &new)
            .changes
            .into_iter()
            .map(|c| (c.path, c.kind, c.description))
            .collect();
        let want: Vec<(String, ChangeKind, String)> = want
            .iter()
            .map(|(p, k, d)| (p.to_string(), *k, d.to_string()))
            .collect();
        assert_eq!(got, want, "{old} -> {new}");
    };
    exact(
        json!({"type": "string"}),
        json!({"type": "number"}),
        &[("type", B, r#"Type changed from "string" to "number""#)],
    );
    exact(
        json!({"type": "string", "format": "email"}),
        json!({"type": "string"}),
        &[("format", N, r#"Format changed from "email" to (none)"#)],
    );
    exact(
        json!({}),
        json!({"pattern": "^a"}),
        &[("pattern", B, r#"Pattern changed from (none) to "^a""#)],
    );
    exact(
        json!({"minimum": 1}),
        json!({}),
        &[("minimum", N, "minimum constraint removed (was 1)")],
    );
    exact(
        json!({}),
        json!({"maximum": 1}),
        &[("maximum", B, "maximum constraint added: 1")],
    );
    exact(
        json!({"maxLength": 3}),
        json!({"maxLength": 2}),
        &[("maxLength", B, "maxLength changed from 3 to 2")],
    );
    // The same bound written another way (or off by less than f64::EPSILON) is no change.
    exact(json!({"minimum": 1}), json!({"minimum": 1.0}), &[]);
    exact(
        json!({"minimum": 1.0}),
        json!({"minimum": 1.0000000000000002}),
        &[],
    );
    exact(
        json!({}),
        json!({"multipleOf": 2}),
        &[("multipleOf", B, "multipleOf constraint added: 2")],
    );
    exact(
        json!({"formatMinimum": "2020-01-01"}),
        json!({}),
        &[(
            "formatMinimum",
            N,
            r#"formatMinimum constraint removed (was "2020-01-01")"#,
        )],
    );
    exact(
        json!({"formatMaximum": "2021-01-01"}),
        json!({"formatMaximum": "2020-01-01"}),
        &[(
            "formatMaximum",
            B,
            "formatMaximum changed from 2021-01-01 to 2020-01-01",
        )],
    );
    exact(
        json!({}),
        json!({"x-minDecimal": "1.5"}),
        &[("x-minDecimal", B, r#"x-minDecimal constraint added: "1.5""#)],
    );
    exact(
        json!({"x-maxDecimal": "2"}),
        json!({"x-maxDecimal": "1.5"}),
        &[("x-maxDecimal", B, "x-maxDecimal changed from 2 to 1.5")],
    );
    // A decimal bound rewritten to the same number accepts the same values.
    exact(
        json!({"x-minDecimal": "2"}),
        json!({"x-minDecimal": "2.0"}),
        &[("x-minDecimal", N, "x-minDecimal changed from 2 to 2")],
    );
    exact(
        json!({"x-maxDecimal": "2.0"}),
        json!({"x-maxDecimal": "2"}),
        &[("x-maxDecimal", N, "x-maxDecimal changed from 2 to 2")],
    );
    exact(
        json!({}),
        json!({"enum": [1]}),
        &[("enum", B, "Enum constraint added")],
    );
    exact(
        json!({"enum": [1]}),
        json!({}),
        &[("enum", N, "Enum constraint removed")],
    );
    // Type arrays are sets: disjoint sets are incompatible.
    exact(
        json!({"type": ["string", "null"]}),
        json!({"type": ["number", "null"]}),
        &[(
            "type",
            B,
            r#"Type changed from ["string","null"] to ["number","null"]"#,
        )],
    );
    // A widened type still compares the other keywords.
    exact(
        json!({"type": "string", "minLength": 1}),
        json!({"type": ["string", "null"], "minLength": 2}),
        &[
            (
                "type",
                N,
                r#"Type changed from "string" to ["string","null"]"#,
            ),
            ("minLength", B, "minLength changed from 1 to 2"),
        ],
    );
    // Same branch count: only the changed branch is reported.
    exact(
        json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
        json!({"anyOf": [{"type": "string", "minLength": 1}, {"type": "number"}]}),
        &[("anyOf[0].minLength", B, "minLength constraint added: 1")],
    );
    exact(
        json!({"anyOf": [{"type": "string"}, {"type": "number"}, {"type": "boolean"}]}),
        json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
        &[("anyOf", B, "1 branch(es) removed from anyOf")],
    );
    exact(
        json!({"allOf": [{"minimum": 0}, {"maximum": 9}]}),
        json!({"allOf": [{"minimum": 0}]}),
        &[("allOf", N, "1 branch(es) removed from allOf")],
    );
    // `additionalProperties: true` and absent both allow anything.
    exact(
        json!({"additionalProperties": true}),
        json!({}),
        &[(
            "additionalProperties",
            N,
            "additionalProperties changed from true to (none)",
        )],
    );
    // An existing field becoming required, or optional again.
    exact(
        obj(json!({"type": "string"}), false),
        obj(json!({"type": "string"}), true),
        &[("required[a]", B, r#"Field "a" is now required"#)],
    );
    exact(
        obj(json!({"type": "string"}), true),
        obj(json!({"type": "string"}), false),
        &[("required[a]", N, r#"Field "a" is no longer required"#)],
    );
    // A removed required property is reported once, as a removed property.
    exact(
        obj(json!({"type": "string"}), true),
        json!({"type": "object", "properties": {}}),
        &[("properties.a", B, r#"Property "a" removed"#)],
    );

    // `multipleOf` agrees with the runtime check: the change is non-breaking exactly when
    // the old divisor is itself accepted by the new one (same relative tolerance).
    for (old, new) in [
        (2.0, 4.0),
        (4.0, 2.0),
        (0.3, 0.1),
        (100000.00001, 1.0),
        (2.0, 3.0),
    ] {
        let d = diff_schemas(&json!({"multipleOf": old}), &json!({"multipleOf": new}));
        let accepted = vld::number()
            .multiple_of(new)
            .parse_value(&json!(old))
            .is_ok();
        assert_eq!(d.has_breaking(), !accepted, "{old} -> {new}: {d}");
    }
}

/// Given a request body `{ name }` that evolves over releases
/// When a required `email` is added, an optional `age` is added, a property is removed, a field
/// becomes optional + nullable, a nested bound tightens (also inside `anyOf` with `null`), a union
/// gains a branch, `additionalProperties` changes, and the diff is printed
/// Then a new required property is one breaking change naming it, an optional one is one
/// non-breaking change, removal is breaking, "optional + nullable" is "Now accepts null" and not a
/// type change, nested changes carry their full dotted path, union branches are diffed by index,
/// a stricter `additionalProperties` is breaking and a looser one is not, and the text marks
/// breaking changes.
#[test]
fn evolving_a_request_body() {
    let base =
        json!({"type": "object", "required": ["name"], "properties": {"name": {"type": "string"}}});
    let add_required = json!({
        "type": "object", "required": ["name", "email"],
        "properties": {"name": {"type": "string"}, "email": {"type": "string"}}
    });
    let d = diff_schemas(&base, &add_required);
    assert!(d.has_breaking());
    assert!(
        d.breaking_changes()
            .iter()
            .any(|c| c.description.contains("email"))
    );

    let add_optional = json!({
        "type": "object", "required": ["name"],
        "properties": {"name": {"type": "string"}, "age": {"type": "integer"}}
    });
    let d = diff_schemas(&base, &add_optional);
    assert!(!d.has_breaking());
    assert_eq!(d.non_breaking_changes().len(), 1);

    assert!(diff_schemas(&add_required, &base).has_breaking());

    // A new required property is reported once.
    let d = diff_schemas(
        &json!({"type": "object", "properties": {}}),
        &obj(json!({"type": "string"}), true),
    );
    assert_eq!(d.changes.len(), 1, "{d}");
    assert_eq!(d.changes[0].kind, ChangeKind::Breaking);

    let d = diff_schemas(
        &obj(json!({"type": "string"}), true),
        &obj(
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            false,
        ),
    );
    assert!(!d.has_breaking(), "{d}");
    assert!(
        d.changes
            .iter()
            .any(|c| c.description == "Now accepts null")
    );

    let nested = |min: u64| {
        json!({"type": "object", "required": ["addr"], "properties": {"addr": {
            "type": "object", "required": ["city"],
            "properties": {"city": {"type": "string", "minLength": min}}
        }}})
    };
    assert_eq!(
        summary(&diff_schemas(&nested(1), &nested(3))),
        [(
            "properties.addr.properties.city.minLength".to_string(),
            ChangeKind::Breaking
        )]
    );
    let wrapped = |max: u64| {
        obj(
            json!({"anyOf": [{"type": "string", "maxLength": max}, {"type": "null"}]}),
            false,
        )
    };
    assert_eq!(
        summary(&diff_schemas(&wrapped(10), &wrapped(5))),
        [("properties.a.maxLength".to_string(), ChangeKind::Breaking)]
    );

    let d = diff_schemas(
        &json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
        &json!({"anyOf": [{"type": "string", "minLength": 1}, {"type": "number"}, {"type": "boolean"}]}),
    );
    assert!(d.changes.iter().any(|c| c.path == "anyOf[0].minLength"));
    assert!(
        d.changes
            .iter()
            .any(|c| c.path == "anyOf" && c.kind == ChangeKind::NonBreaking)
    );

    let extra = |ap: Option<Value>| {
        let mut v = json!({"type": "object"});
        if let Some(ap) = ap {
            v["additionalProperties"] = ap;
        }
        v
    };
    let string = Some(json!({"type": "string"}));
    let number = Some(json!({"type": "number"}));
    assert!(diff_schemas(&extra(string), &extra(number.clone())).has_breaking());
    assert!(diff_schemas(&extra(None), &extra(number.clone())).has_breaking());
    assert!(!diff_schemas(&extra(number), &extra(None)).has_breaking());

    let text = diff_schemas(&json!({"type": "string"}), &json!({"type": "number"})).to_string();
    assert!(text.contains("BREAKING"), "{text}");
    assert!(text.contains("type"), "{text}");
}
