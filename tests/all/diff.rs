//! `vld::diff::diff_schemas`: breaking vs non-breaking JSON Schema changes.

use serde_json::{json, Value};
use vld::diff::{diff_schemas, ChangeKind, SchemaDiff};

fn obj(prop: Value, required: bool) -> Value {
    let mut v = json!({"type": "object", "properties": {"a": prop}});
    if required {
        v["required"] = json!(["a"]);
    }
    v
}

/// `(path, kind)` of every change.
fn summary(d: &SchemaDiff) -> Vec<(String, ChangeKind)> {
    d.changes
        .iter()
        .map(|c| (c.path.clone(), c.kind))
        .collect()
}

#[test]
fn identical_schemas_have_no_changes() {
    let s = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    let d = diff_schemas(&s, &s);
    assert!(d.changes.is_empty());
    assert!(!d.has_breaking());
}

/// `(old, new, breaking?)` for single-keyword changes.
#[test]
fn keyword_direction_table() {
    let cases = [
        (json!({"type": "string"}), json!({"type": "number"}), true),
        (
            json!({"type": "number", "minimum": 0}),
            json!({"type": "number", "minimum": 5}),
            true,
        ),
        (
            json!({"type": "number", "minimum": 5}),
            json!({"type": "number", "minimum": 0}),
            false,
        ),
        (
            json!({"type": "number", "maximum": 5}),
            json!({"type": "number", "maximum": 3}),
            true,
        ),
        (
            json!({"type": "string", "maxLength": 10}),
            json!({"type": "string", "maxLength": 20}),
            false,
        ),
        (
            json!({"type": "string"}),
            json!({"type": "string", "format": "email"}),
            true,
        ),
        (
            json!({"type": "string", "format": "email"}),
            json!({"type": "string"}),
            false,
        ),
        (
            json!({"enum": ["a", "b", "c"]}),
            json!({"enum": ["a", "b"]}),
            true,
        ),
        (
            json!({"enum": ["a", "b"]}),
            json!({"enum": ["a", "b", "c"]}),
            false,
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            json!({"type": "string"}),
            true,
        ),
        (
            json!({"$ref": "#/components/schemas/A"}),
            json!({"$ref": "#/components/schemas/B"}),
            true,
        ),
    ];
    for (old, new, breaking) in cases {
        let d = diff_schemas(&old, &new);
        assert!(!d.changes.is_empty(), "{old} -> {new}: no change detected");
        assert_eq!(d.has_breaking(), breaking, "{old} -> {new}: {d}");
        assert_eq!(
            d.breaking_changes().len() + d.non_breaking_changes().len(),
            d.changes.len()
        );
        if !breaking {
            assert!(d.breaking_changes().is_empty());
            assert_eq!(d.non_breaking_changes().len(), d.changes.len());
        }
    }
}

#[test]
fn required_and_property_changes() {
    let base =
        json!({"type": "object", "required": ["name"], "properties": {"name": {"type": "string"}}});
    let add_required = json!({
        "type": "object", "required": ["name", "email"],
        "properties": {"name": {"type": "string"}, "email": {"type": "string"}}
    });
    let d = diff_schemas(&base, &add_required);
    assert!(d.has_breaking());
    assert!(d
        .breaking_changes()
        .iter()
        .any(|c| c.description.contains("email")));

    let add_optional = json!({
        "type": "object", "required": ["name"],
        "properties": {"name": {"type": "string"}, "age": {"type": "integer"}}
    });
    let d = diff_schemas(&base, &add_optional);
    assert!(!d.has_breaking());
    assert_eq!(d.non_breaking_changes().len(), 1);

    // Removing a property is breaking.
    assert!(diff_schemas(&add_required, &base).has_breaking());

    // A new required property is reported once.
    let d = diff_schemas(
        &json!({"type": "object", "properties": {}}),
        &obj(json!({"type": "string"}), true),
    );
    assert_eq!(d.changes.len(), 1, "{d}");
    assert_eq!(d.changes[0].kind, ChangeKind::Breaking);
}

#[test]
fn making_a_field_optional_is_not_a_type_change() {
    let old = obj(json!({"type": "string"}), true);
    let new = obj(
        json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
        false,
    );
    let d = diff_schemas(&old, &new);
    assert!(!d.has_breaking(), "{d}");
    assert!(d
        .changes
        .iter()
        .any(|c| c.description == "Now accepts null"));
}

#[test]
fn nested_and_wrapped_changes_have_full_paths() {
    let inner = |min: u64| {
        json!({"type": "object", "required": ["addr"], "properties": {"addr": {
            "type": "object", "required": ["city"],
            "properties": {"city": {"type": "string", "minLength": min}}
        }}})
    };
    let d = diff_schemas(&inner(1), &inner(3));
    assert_eq!(
        summary(&d),
        [(
            "properties.addr.properties.city.minLength".to_string(),
            ChangeKind::Breaking
        )]
    );

    let old = obj(
        json!({"anyOf": [{"type": "string", "maxLength": 10}, {"type": "null"}]}),
        false,
    );
    let new = obj(
        json!({"anyOf": [{"type": "string", "maxLength": 5}, {"type": "null"}]}),
        false,
    );
    assert_eq!(
        summary(&diff_schemas(&old, &new)),
        [("properties.a.maxLength".to_string(), ChangeKind::Breaking)]
    );
}

#[test]
fn union_branches_are_diffed() {
    let d = diff_schemas(
        &json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
        &json!({"anyOf": [{"type": "string", "minLength": 1}, {"type": "number"}, {"type": "boolean"}]}),
    );
    assert!(d.changes.iter().any(|c| c.path == "anyOf[0].minLength"));
    assert!(d
        .changes
        .iter()
        .any(|c| c.path == "anyOf" && c.kind == ChangeKind::NonBreaking));
}

#[test]
fn additional_properties_schema_is_diffed() {
    let s = |ap: Option<Value>| {
        let mut v = json!({"type": "object"});
        if let Some(ap) = ap {
            v["additionalProperties"] = ap;
        }
        v
    };
    let string = Some(json!({"type": "string"}));
    let number = Some(json!({"type": "number"}));
    assert!(diff_schemas(&s(string), &s(number.clone())).has_breaking());
    assert!(diff_schemas(&s(None), &s(number.clone())).has_breaking());
    assert!(!diff_schemas(&s(number), &s(None)).has_breaking());
}

#[test]
fn display_marks_breaking_changes() {
    let d = diff_schemas(&json!({"type": "string"}), &json!({"type": "number"}));
    let text = d.to_string();
    assert!(text.contains("BREAKING"), "{text}");
    assert!(text.contains("type"), "{text}");
}
