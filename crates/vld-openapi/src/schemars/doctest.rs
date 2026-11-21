
// vld → schemars: `CreateUser` implements `schemars::JsonSchema`.
vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}
impl_json_schema!(CreateUser);

// With a custom schema name.
vld::schema! {
    #[derive(Debug)]
    pub struct Req { pub x: String => vld::string() }
}
impl_json_schema!(Req, "CreateUserRequest");
assert_eq!(<Req as schemars::JsonSchema>::schema_name(), "CreateUserRequest");

// schemars → vld: validate an instance, validate + deserialize JSON, or go
// through `vld::schema::VldParse` (vld extractors).
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Item { name: String, qty: u32 }
impl_vld_parse!(Item);

let item = Item { name: "Widget".into(), qty: 5 };
assert!(item.vld_validate().is_ok());
let json = serde_json::json!({"name": "Widget", "qty": 5});
assert_eq!(Item::vld_parse(&json).unwrap().name, "Widget");
use vld::schema::VldParse;
assert_eq!(Item::vld_parse_value(&json).unwrap().qty, 5);

// Conversions: non-schemas fail closed (or are an error with `try_`).
let schema = vld_to_schemars(&serde_json::json!({"type": "string", "minLength": 2}));
assert_eq!(schema.get("type").unwrap(), "string");
assert_eq!(vld_to_schemars(&serde_json::json!(42)).as_bool(), Some(false));
assert!(try_vld_to_schemars(&serde_json::json!("string")).is_err());
use vld::openapi::JsonSchema as _;
let email = vld_schema_to_schemars(&vld::string().email().json_schema());
assert_eq!(email.get("format").unwrap(), "email");
assert!(validate_with_schemars(&schema, &serde_json::json!("hello")).is_ok());
assert!(validate_with_schemars(&schema, &serde_json::json!("x")).is_err());

assert!(schemars_to_json(&generate_schemars::<String>()).is_object());
assert_eq!(generate_from_schemars::<String>()["type"], "string");

let a = vld_to_schemars(&serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}));
let b = vld_to_schemars(&serde_json::json!({"type": "object", "properties": {"age": {"type": "integer"}}}));
assert!(merge_schemas(&a, &b).get("allOf").is_some());
