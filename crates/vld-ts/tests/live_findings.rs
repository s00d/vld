//! Regression tests for the live end-to-end findings (0.5.0).

use vld::schema::VldSchema;

/// vld applies `with_default` to `null` too; the generated code must as well
/// (checked against zod 3 / valibot 1 with node).
#[test]
fn default_also_replaces_null() {
    let schema = vld::string().min(1).with_default("user".to_string());
    assert_eq!(
        schema.parse_value(&serde_json::Value::Null).unwrap(),
        "user"
    );
    assert_eq!(
        vld_ts::to_zod(&schema),
        r#"z.string().min(1).nullish().transform((v) => v ?? "user")"#
    );
    assert_eq!(
        vld_ts::to_valibot(&schema),
        r#"v.nullish(v.pipe(v.string(), v.minLength(1)), "user")"#
    );
}
