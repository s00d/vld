use vld::prelude::*;

// ---------------------------------------------------------------------------
// schema! with `as "key"` rename
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct RenamedUser {
        pub first_name: String as "firstName" => vld::string().min(1),
        pub last_name: String as "lastName" => vld::string().min(1),
        pub email_address: String => vld::string().email(),
    }
}

#[test]
fn parse_renamed_fields() {
    let json = r#"{
        "firstName": "John",
        "lastName": "Doe",
        "email_address": "john@example.com"
    }"#;

    let user = RenamedUser::parse(json).unwrap();
    assert_eq!(user.first_name, "John");
    assert_eq!(user.last_name, "Doe");
    assert_eq!(user.email_address, "john@example.com");
}

#[test]
fn parse_renamed_fields_uses_json_key_not_rust_name() {
    // Using Rust field names should fail (field not found -> null)
    let json = r#"{
        "first_name": "John",
        "last_name": "Doe",
        "email_address": "john@example.com"
    }"#;

    let err = RenamedUser::parse(json).unwrap_err();
    // firstName and lastName are missing -> validation error
    assert!(err.issues.len() >= 2);
}

#[test]
fn error_path_uses_json_key() {
    let json = r#"{
        "firstName": "",
        "lastName": "Doe",
        "email_address": "john@example.com"
    }"#;

    let err = RenamedUser::parse(json).unwrap_err();
    let paths: Vec<String> = err
        .issues
        .iter()
        .map(|i| i.path.iter().map(|p| p.to_string()).collect())
        .collect();

    // Error path should use "firstName", not "first_name"
    assert!(
        paths.iter().any(|p| p.contains("firstName")),
        "expected firstName in paths: {:?}",
        paths
    );
}

// ---------------------------------------------------------------------------
// Mixed: some fields renamed, some not
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct MixedRename {
        pub name: String => vld::string().min(1),
        pub zip_code: String as "zipCode" => vld::string().len(5),
    }
}

#[test]
fn mixed_rename_works() {
    let json = r#"{"name": "Alice", "zipCode": "12345"}"#;
    let m = MixedRename::parse(json).unwrap();
    assert_eq!(m.name, "Alice");
    assert_eq!(m.zip_code, "12345");
}

// ---------------------------------------------------------------------------
// impl_validate_fields! with rename
// ---------------------------------------------------------------------------

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

#[test]
fn validate_fields_uses_rename() {
    let json = r#"{"userName": "Al", "age": 25}"#;
    let fields = RenamedProfile::validate_fields(json).unwrap();
    assert_eq!(fields[0].name, "userName");
    assert!(fields[0].is_ok());
}

#[test]
fn parse_lenient_uses_rename() {
    let json = r#"{"userName": "X", "age": 25}"#;
    let result = RenamedProfile::parse_lenient(json).unwrap();
    // userName is too short (min 2), should be defaulted
    assert!(result.has_errors());
    let field = result.field("userName");
    assert!(field.is_some());
    assert!(field.unwrap().is_err());
}

// ---------------------------------------------------------------------------
// schema_validated! with rename
// ---------------------------------------------------------------------------

vld::schema_validated! {
    #[derive(Debug, Default, serde::Serialize)]
    pub struct ValidatedRenamed {
        pub full_name: String as "fullName" => vld::string().min(1),
        pub score: f64 => vld::number().min(0.0).max(100.0),
    }
}

#[test]
fn schema_validated_rename() {
    let json = r#"{"fullName": "Bob", "score": 95.5}"#;
    let v = ValidatedRenamed::parse(json).unwrap();
    assert_eq!(v.full_name, "Bob");

    let result = ValidatedRenamed::parse_lenient(json).unwrap();
    assert!(result.is_valid());
}

// ---------------------------------------------------------------------------
// VldParse trait
// ---------------------------------------------------------------------------

#[test]
fn vld_parse_trait_implemented() {
    let value = serde_json::json!({
        "firstName": "Jane",
        "lastName": "Smith",
        "email_address": "jane@example.com"
    });

    let user = <RenamedUser as VldParse>::vld_parse_value(&value).unwrap();
    assert_eq!(user.first_name, "Jane");
}

// ---------------------------------------------------------------------------
// OpenAPI with rename
// ---------------------------------------------------------------------------

#[cfg(feature = "openapi")]
#[test]
fn json_schema_uses_renamed_keys() {
    let schema = RenamedUser::json_schema();
    let props = schema["properties"].as_object().unwrap();

    assert!(
        props.contains_key("firstName"),
        "expected firstName in properties"
    );
    assert!(
        props.contains_key("lastName"),
        "expected lastName in properties"
    );
    assert!(
        props.contains_key("email_address"),
        "expected email_address in properties"
    );
    assert!(
        !props.contains_key("first_name"),
        "should not contain first_name"
    );

    let required = schema["required"].as_array().unwrap();
    let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(required_strs.contains(&"firstName"));
    assert!(required_strs.contains(&"lastName"));
}
