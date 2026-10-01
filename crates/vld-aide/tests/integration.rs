use serde_json::json;
use vld::schema::VldSchema as _;
use vld_aide::schemars;
use vld_aide::schemars::JsonSchema;
use vld_aide::{impl_json_schema, vld_to_schemars};

// ---- vld_to_schemars tests ----

#[test]
fn string_type() {
    let s = vld_to_schemars(&json!({"type": "string"}));
    assert_eq!(s.get("type").unwrap(), "string");
}

#[test]
fn string_with_constraints() {
    let s = vld_to_schemars(&json!({
        "type": "string",
        "minLength": 2,
        "maxLength": 50,
        "format": "email"
    }));
    assert_eq!(s.get("type").unwrap(), "string");
    assert_eq!(s.get("minLength").unwrap(), 2);
    assert_eq!(s.get("maxLength").unwrap(), 50);
    assert_eq!(s.get("format").unwrap(), "email");
}

#[test]
fn integer_type() {
    let s = vld_to_schemars(&json!({"type": "integer", "minimum": 0, "maximum": 100}));
    assert_eq!(s.get("type").unwrap(), "integer");
    assert_eq!(s.get("minimum").unwrap(), 0);
    assert_eq!(s.get("maximum").unwrap(), 100);
}

#[test]
fn boolean_type() {
    let s = vld_to_schemars(&json!({"type": "boolean"}));
    assert_eq!(s.get("type").unwrap(), "boolean");
}

#[test]
fn object_with_properties() {
    let s = vld_to_schemars(&json!({
        "type": "object",
        "required": ["name", "email"],
        "properties": {
            "name": {"type": "string", "minLength": 1},
            "email": {"type": "string", "format": "email"}
        }
    }));
    assert_eq!(s.get("type").unwrap(), "object");
    let props = s.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("name"));
    assert!(props.contains_key("email"));
    let req = s.get("required").unwrap().as_array().unwrap();
    assert!(req.contains(&json!("name")));
    assert!(req.contains(&json!("email")));
}

#[test]
fn array_type() {
    let s = vld_to_schemars(&json!({
        "type": "array",
        "items": {"type": "string"},
        "minItems": 1,
        "maxItems": 10
    }));
    assert_eq!(s.get("type").unwrap(), "array");
    assert_eq!(s.get("minItems").unwrap(), 1);
    assert_eq!(s.get("maxItems").unwrap(), 10);
}

#[test]
fn one_of() {
    let s = vld_to_schemars(&json!({
        "oneOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    }));
    let items = s.get("oneOf").unwrap().as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn bool_value_fallback() {
    let s = vld_to_schemars(&json!(true));
    assert!(s.as_bool() == Some(true));
}

#[test]
fn non_object_returns_default() {
    let s = vld_to_schemars(&json!("not a schema"));
    let _ = s; // just assert no panic
}

// ---- impl_json_schema! tests ----

vld::schema! {
    #[derive(Debug)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}

impl_json_schema!(TestUser);

#[test]
fn impl_json_schema_name() {
    assert_eq!(TestUser::schema_name(), "TestUser");
}

#[test]
fn impl_json_schema_id() {
    let id = TestUser::schema_id();
    assert!(id.contains("TestUser"));
}

#[test]
fn impl_json_schema_generates_schema() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <TestUser as JsonSchema>::json_schema(&mut gen);
    assert_eq!(schema.get("type").unwrap(), "object");

    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("name"));
    assert!(props.contains_key("email"));
    assert!(props.contains_key("age"));

    let name_schema = &props["name"];
    assert_eq!(name_schema["type"], "string");
    assert_eq!(name_schema["minLength"], 2);
    assert_eq!(name_schema["maxLength"], 50);

    let email_schema = &props["email"];
    assert_eq!(email_schema["format"], "email");
}

// Custom name

vld::schema! {
    #[derive(Debug)]
    pub struct ReqBody {
        pub x: String => vld::string().min(1),
    }
}

impl_json_schema!(ReqBody, "CreateRequest");

#[test]
fn impl_json_schema_custom_name() {
    assert_eq!(ReqBody::schema_name(), "CreateRequest");
}

#[test]
fn impl_json_schema_custom_name_schema_works() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <ReqBody as JsonSchema>::json_schema(&mut gen);
    assert_eq!(schema.get("type").unwrap(), "object");
}

// ---- derive(Validate) + impl_json_schema! ----

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

#[test]
fn derive_impl_json_schema_works() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <DeriveUser as JsonSchema>::json_schema(&mut gen);
    assert_eq!(schema.get("type").unwrap(), "object");

    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert_eq!(props["name"]["type"], "string");
    assert_eq!(props["name"]["minLength"], 2);
    assert_eq!(props["name"]["maxLength"], 50);
    assert_eq!(props["email"]["format"], "email");
}

#[test]
fn derive_impl_json_schema_name() {
    assert_eq!(DeriveUser::schema_name(), "DeriveUser");
}

#[derive(Debug, serde::Deserialize, vld::Validate)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DeriveRenamedRequest {
    #[vld(vld::string().min(1).max(255))]
    first_name: String,
    #[vld(vld::string().email())]
    email_address: String,
    #[vld(vld::number().int().non_negative().min(1).max(9999))]
    street_number: i64,
    #[vld(vld::boolean())]
    is_active: bool,
}

impl_json_schema!(DeriveRenamedRequest);

#[test]
fn derive_rename_all_camel_case_schema() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <DeriveRenamedRequest as JsonSchema>::json_schema(&mut gen);
    assert_eq!(schema.get("type").unwrap(), "object");

    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(
        props.contains_key("firstName"),
        "firstName property missing"
    );
    assert!(
        props.contains_key("emailAddress"),
        "emailAddress property missing"
    );
    assert!(
        props.contains_key("streetNumber"),
        "streetNumber property missing"
    );
    assert!(props.contains_key("isActive"), "isActive property missing");

    assert!(
        !props.contains_key("first_name"),
        "snake_case key should not exist"
    );
}

// ---- Round-trip: vld json_schema -> schemars ----

#[test]
fn roundtrip_string_schema() {
    let vld_schema = vld::string().min(3).max(100).email();
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let schemars_schema = vld_to_schemars(&js);
    assert_eq!(schemars_schema.get("type").unwrap(), "string");
    assert_eq!(schemars_schema.get("minLength").unwrap(), 3);
    assert_eq!(schemars_schema.get("maxLength").unwrap(), 100);
    assert_eq!(schemars_schema.get("format").unwrap(), "email");
}

#[test]
fn roundtrip_number_schema() {
    let vld_schema = vld::number().min(0.0).max(100.0);
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let schemars_schema = vld_to_schemars(&js);
    assert_eq!(schemars_schema.get("type").unwrap(), "number");
}

#[test]
fn roundtrip_full_struct() {
    let js = TestUser::json_schema();
    let schemars_schema = vld_to_schemars(&js);
    assert_eq!(schemars_schema.get("type").unwrap(), "object");

    let req = schemars_schema.get("required").unwrap().as_array().unwrap();
    assert!(req.contains(&json!("name")));
    assert!(req.contains(&json!("email")));
    assert!(!req.contains(&json!("age")));
}

vld::schema! {
    #[derive(Debug)]
    pub struct AideCreateTicket {
        pub title: String => vld::string()
            .min(3)
            .max(120)
            .describe("ticket title")
            .message("bad title"),
        pub body: String => vld::string().min(1),
        pub priority: String => vld::string().with_default("normal".into()),
        pub labels: Option<Vec<String>> => vld::array(vld::string().min(1)).optional(),
        pub due_at: Option<String> => vld::string().nullish().describe("ISO date"),
        pub reporter: Option<String> => vld::string()
            .email()
            .nullable()
            .describe("reporter email"),
        pub meta: Option<String> => vld::string()
            .optional()
            .describe("freeform")
            .catch(None),
    }
}

impl_json_schema!(AideCreateTicket);

#[derive(Debug, vld::Validate)]
#[allow(dead_code)]
struct AideDerivePatch {
    #[vld(vld::string().min(3).describe("title"))]
    title: String,
    #[vld(vld::string().optional().describe("body"))]
    body: Option<String>,
    #[vld(vld::string().with_default("normal".into()))]
    priority: String,
    #[vld(vld::string().nullish())]
    assignee: Option<String>,
    #[vld(vld::string().nullable().describe("cursor"))]
    cursor: Option<String>,
}

impl_json_schema!(AideDerivePatch);

fn aide_required(schema: &serde_json::Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

/// End-to-end aide/schemars: schema! + derive required matrix with wrappers.
#[test]
fn openapi_required_matrix_roundtrip_schema_and_derive() {
    let js = AideCreateTicket::json_schema();
    assert_eq!(
        aide_required(&js),
        vec!["title", "body", "reporter"],
        "nullable stays required; optional/nullish/default omitted"
    );
    assert_eq!(js["properties"]["title"]["description"], "ticket title");
    assert_eq!(js["properties"]["title"]["maxLength"], 120);
    assert_eq!(js["properties"]["priority"]["type"], "string");
    assert!(js["properties"]["labels"]["oneOf"].is_array());
    assert!(js["properties"]["due_at"]["oneOf"].is_array());
    assert_eq!(js["properties"]["due_at"]["description"], "ISO date");
    assert!(js["properties"]["reporter"]["oneOf"].is_array());
    assert_eq!(
        js["properties"]["reporter"]["description"],
        "reporter email"
    );
    assert!(js["properties"]["meta"]["oneOf"].is_array());

    let schemars = vld_to_schemars(&js);
    assert_eq!(schemars.get("type").unwrap(), "object");
    assert_eq!(
        aide_required(&serde_json::to_value(&schemars).unwrap()),
        vec!["title", "body", "reporter"]
    );

    let mut gen = schemars::SchemaGenerator::default();
    let generated = <AideCreateTicket as JsonSchema>::json_schema(&mut gen);
    let generated_json = serde_json::to_value(&generated).unwrap();
    assert_eq!(
        aide_required(&generated_json),
        vec!["title", "body", "reporter"]
    );
    assert_eq!(generated_json["properties"]["title"]["minLength"], 3);
    assert!(generated_json["properties"]["labels"]["oneOf"].is_array());
    assert!(generated_json["properties"]["reporter"]["oneOf"].is_array());

    let derive_js = AideDerivePatch::json_schema();
    assert_eq!(
        aide_required(&derive_js),
        vec!["title", "cursor"],
        "derive: optional/nullish/default omitted, nullable kept"
    );
    assert!(derive_js["properties"]["body"]["oneOf"].is_array());
    assert_eq!(derive_js["properties"]["priority"]["type"], "string");
    assert!(derive_js["properties"]["assignee"]["oneOf"].is_array());
    assert!(derive_js["properties"]["cursor"]["oneOf"].is_array());

    let mut gen2 = schemars::SchemaGenerator::default();
    let derive_generated = <AideDerivePatch as JsonSchema>::json_schema(&mut gen2);
    let derive_json = serde_json::to_value(&derive_generated).unwrap();
    assert_eq!(aide_required(&derive_json), vec!["title", "cursor"]);
    assert_eq!(derive_json["properties"]["title"]["description"], "title");
    assert_eq!(derive_json["properties"]["cursor"]["description"], "cursor");
}

// ---- Nested schemas ----

vld::schema! {
    #[derive(Debug)]
    pub struct AideAddress {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}

impl_json_schema!(AideAddress);

vld::schema! {
    #[derive(Debug)]
    pub struct AideOrder {
        pub name: String => vld::string().min(1),
        pub address: AideAddress => vld::nested!(AideAddress),
    }
}

impl_json_schema!(AideOrder);

#[test]
fn nested_schema_ref_in_schemars() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <AideOrder as schemars::JsonSchema>::json_schema(&mut gen);
    let props = schema.get("properties").unwrap();
    assert_eq!(props["address"]["$ref"], "#/components/schemas/AideAddress");
}

#[test]
fn nested_schema_registered_in_definitions() {
    let mut gen = schemars::SchemaGenerator::default();
    let _schema = <AideOrder as schemars::JsonSchema>::json_schema(&mut gen);
    let defs = gen.definitions();
    assert!(
        defs.contains_key("AideAddress"),
        "AideAddress should be in definitions, got: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
    let addr = &defs["AideAddress"];
    assert_eq!(addr["type"], "object");
    assert!(addr["properties"]["city"].is_object());
}

/// Regression for https://github.com/s00d/vld/issues/5 — grandchild schemas
/// must appear in aide/schemars definitions transitively.
#[test]
fn transitive_nested_schema_registered_in_definitions() {
    vld::schema! {
        #[derive(Debug)]
        pub struct AideBreak {
            pub label: String => vld::string().min(1),
        }
    }
    impl_json_schema!(AideBreak);

    vld::schema! {
        #[derive(Debug)]
        pub struct AideDay {
            pub breaks: Option<Vec<AideBreak>> =>
                vld::array(vld::nested!(AideBreak)).optional(),
        }
    }
    impl_json_schema!(AideDay);

    vld::schema! {
        #[derive(Debug)]
        pub struct AideSchedule {
            pub days: Vec<AideDay> => vld::array(vld::nested!(AideDay)),
        }
    }
    impl_json_schema!(AideSchedule);

    let mut gen = schemars::SchemaGenerator::default();
    let _schema = <AideSchedule as schemars::JsonSchema>::json_schema(&mut gen);
    let defs = gen.definitions();

    assert!(
        defs.contains_key("AideDay"),
        "child AideDay missing: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
    assert!(
        defs.contains_key("AideBreak"),
        "grandchild AideBreak missing: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
}
