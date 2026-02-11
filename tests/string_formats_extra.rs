use vld::prelude::*;

// ---- CUID2 ----

#[test]
fn cuid2_valid() {
    let schema = vld::string().cuid2();
    assert!(schema.parse(r#""clh3a8e9g000008l5fkbc1z1n""#).is_ok());
    assert!(schema.parse(r#""abc123def456""#).is_ok());
}

#[test]
fn cuid2_invalid_starts_uppercase() {
    let schema = vld::string().cuid2();
    assert!(schema.parse(r#""ABC123""#).is_err());
}

#[test]
fn cuid2_invalid_has_special() {
    let schema = vld::string().cuid2();
    assert!(schema.parse(r#""abc-123""#).is_err());
}

#[test]
fn cuid2_custom_msg() {
    let schema = vld::string().cuid2_msg("bad cuid2");
    let err = schema.parse(r#""!!""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad cuid2"));
}

// ---- ULID ----

#[test]
fn ulid_valid() {
    let schema = vld::string().ulid();
    // 26 Crockford Base32 chars
    assert!(schema.parse(r#""01ARZ3NDEKTSV4RRFFQ69G5FAV""#).is_ok());
}

#[test]
fn ulid_invalid_length() {
    let schema = vld::string().ulid();
    assert!(schema.parse(r#""01ARZ3""#).is_err());
}

#[test]
fn ulid_invalid_chars() {
    let schema = vld::string().ulid();
    // 'I', 'L', 'O', 'U' are not in Crockford Base32
    assert!(schema.parse(r#""01ARZ3NDEKTSV4RRFFQ69GILOL""#).is_err());
}

#[test]
fn ulid_custom_msg() {
    let schema = vld::string().ulid_msg("bad ulid");
    let err = schema.parse(r#""short""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad ulid"));
}

// ---- Nano ID ----

#[test]
fn nanoid_valid() {
    let schema = vld::string().nanoid();
    assert!(schema.parse(r#""V1StGXR8_Z5jdHi6B-myT""#).is_ok());
}

#[test]
fn nanoid_invalid_special() {
    let schema = vld::string().nanoid();
    assert!(schema.parse(r#""hello world""#).is_err()); // space
}

#[test]
fn nanoid_empty() {
    let schema = vld::string().nanoid();
    assert!(schema.parse(r#""""#).is_err());
}

#[test]
fn nanoid_custom_msg() {
    let schema = vld::string().nanoid_msg("bad nanoid");
    let err = schema.parse(r#""hello world""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad nanoid"));
}

// ---- Emoji ----

#[test]
fn emoji_valid() {
    let schema = vld::string().emoji();
    assert!(schema.parse(r#""hello 😀""#).is_ok());
    assert!(schema.parse(r#""⭐""#).is_ok());
}

#[test]
fn emoji_invalid() {
    let schema = vld::string().emoji();
    assert!(schema.parse(r#""hello world""#).is_err());
}

#[test]
fn emoji_custom_msg() {
    let schema = vld::string().emoji_msg("need emoji");
    let err = schema.parse(r#""plain""#).unwrap_err();
    assert!(err.issues[0].message.contains("need emoji"));
}
