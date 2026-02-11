use serde_json::json;
use vld::prelude::*;

fn animal_schema() -> ZDiscriminatedUnion {
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

#[test]
fn routes_to_dog() {
    let result = animal_schema()
        .parse(r#"{"type":"dog","bark":true}"#)
        .unwrap();
    assert_eq!(result["bark"], json!(true));
}

#[test]
fn routes_to_cat() {
    let result = animal_schema()
        .parse(r#"{"type":"cat","lives":9}"#)
        .unwrap();
    assert_eq!(result["lives"], json!(9));
}

#[test]
fn unknown_variant() {
    let err = animal_schema().parse(r#"{"type":"fish"}"#).unwrap_err();
    assert!(err.issues[0].message.contains("discriminator"));
}

#[test]
fn missing_discriminator_field() {
    let err = animal_schema().parse(r#"{"name":"x"}"#).unwrap_err();
    assert!(err.issues[0].message.contains("discriminator"));
}

#[test]
fn not_an_object() {
    assert!(animal_schema().parse(r#""hello""#).is_err());
}
