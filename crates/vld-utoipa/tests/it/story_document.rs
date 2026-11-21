//! Stories: an API documents its vld types with utoipa — request / response
//! bodies (`ToSchema`, nested components registered transitively), the
//! required matrix, renamed fields and query / path / header / cookie
//! parameters (`IntoParams`) — and the assembled OpenAPI document is complete.

use serde_json::{Map, Value, json};

use vld_utoipa::utoipa::openapi::path::ParameterIn;
use vld_utoipa::utoipa::openapi::{RefOr, Required, schema::Schema};
use vld_utoipa::utoipa::{IntoParams, PartialSchema, ToSchema};
use vld_utoipa::{
    impl_to_schema, json_schema_to_params, json_schema_to_schema, parameter_in_from_str,
    resolve_parameter_in,
};

vld::schema! {
    #[derive(Debug)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}
impl_to_schema!(Address);

vld::schema! {
    #[derive(Debug)]
    pub struct Order {
        pub name: String => vld::string().min(1),
        pub shipping: Address => vld::nested!(Address),
        pub billing: Address => vld::nested!(Address),
    }
}
impl_to_schema!(Order);

vld::schema! {
    #[derive(Debug)]
    pub struct Warehouse {
        pub name: String => vld::string().min(1),
        pub addresses: Vec<Address> => vld::array(vld::nested!(Address)),
    }
}
impl_to_schema!(Warehouse);

vld::schema! {
    #[derive(Debug)]
    pub struct ScheduleBreakItem {
        pub start_time: String as "startTime" => vld::string().min(1),
        pub end_time: String as "endTime" => vld::string().min(1),
    }
}
impl_to_schema!(ScheduleBreakItem);

vld::schema! {
    #[derive(Debug)]
    pub struct ScheduleDayItem {
        pub day_of_week: i64 as "dayOfWeek" => vld::number().int().min(1).max(7),
        pub breaks: Option<Vec<ScheduleBreakItem>> as "breaks" =>
            vld::array(vld::nested!(ScheduleBreakItem)).optional(),
    }
}
impl_to_schema!(ScheduleDayItem);

vld::schema! {
    #[derive(Debug)]
    pub struct EmployeeScheduleRequest {
        pub name: String => vld::string().min(1),
        pub days: Vec<ScheduleDayItem> => vld::array(vld::nested!(ScheduleDayItem)),
    }
}
impl_to_schema!(EmployeeScheduleRequest);

vld::schema! {
    #[derive(Debug)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}
impl_to_schema!(TestUser);

vld::schema! {
    #[derive(Debug)]
    pub struct ReqBody {
        pub x: String => vld::string().min(1),
    }
}
impl_to_schema!(ReqBody, "CreateRequest");

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
impl_to_schema!(DeriveUser);

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
    #[vld(vld::string().optional())]
    street_number_addition: Option<String>,
    #[vld(vld::boolean())]
    is_active: bool,
}
impl_to_schema!(DeriveRenamedRequest);

fn to_json<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

/// The `components.schemas` an OpenAPI generator collects for `T`: `T` itself
/// under its name plus everything `ToSchema::schemas` registers.
fn components<T: ToSchema>(into: &mut Map<String, Value>) {
    into.insert(T::name().into_owned(), to_json(&T::schema()));
    let mut nested = Vec::new();
    T::schemas(&mut nested);
    for (name, schema) in nested {
        into.insert(name, to_json(&schema));
    }
}

/// Every `$ref` in `v`.
fn refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(r)) = m.get("$ref") {
                out.push(r.clone());
            }
            m.values().for_each(|x| refs(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| refs(x, out)),
        _ => {}
    }
}

fn required(schema: &Value) -> Vec<&str> {
    schema["required"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Given body types with nested, repeated, array-wrapped and transitive
/// `vld::nested!` fields, when an API collects their components, then every
/// `$ref` in the document resolves, each component is its type's converted
/// `json_schema()`, and flat / derive types register nothing extra.
#[test]
fn a_document_built_from_vld_types_is_complete() {
    let mut schemas = Map::new();
    components::<Order>(&mut schemas);
    components::<Warehouse>(&mut schemas);
    components::<EmployeeScheduleRequest>(&mut schemas);
    components::<TestUser>(&mut schemas);
    components::<ReqBody>(&mut schemas);
    components::<DeriveUser>(&mut schemas);
    let doc = json!({"openapi": "3.1.0", "components": {"schemas": schemas}});

    let mut names: Vec<&String> = schemas.keys().collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Address",
            "CreateRequest",
            "DeriveUser",
            "EmployeeScheduleRequest",
            "Order",
            "ScheduleBreakItem",
            "ScheduleDayItem",
            "TestUser",
            "Warehouse"
        ]
    );
    let mut all = Vec::new();
    refs(&doc, &mut all);
    assert!(all.len() >= 5, "{all:?}");
    for r in &all {
        assert!(
            doc.pointer(r.trim_start_matches('#')).is_some(),
            "dangling {r}"
        );
    }

    let order = &schemas["Order"];
    assert_eq!(
        order["properties"]["shipping"]["$ref"],
        "#/components/schemas/Address"
    );
    assert_eq!(
        order["properties"]["billing"]["$ref"],
        "#/components/schemas/Address"
    );
    assert_eq!(
        schemas["Warehouse"]["properties"]["addresses"]["items"]["$ref"],
        "#/components/schemas/Address"
    );
    assert_eq!(
        schemas["ScheduleDayItem"]["properties"]["breaks"]["items"]["$ref"],
        "#/components/schemas/ScheduleBreakItem"
    );
    assert_eq!(
        schemas["ScheduleBreakItem"]["required"],
        json!(["startTime", "endTime"])
    );
    for (name, js) in [
        ("Address", Address::json_schema()),
        ("Order", Order::json_schema()),
        ("TestUser", TestUser::json_schema()),
    ] {
        assert_eq!(
            schemas[name],
            to_json(&json_schema_to_schema(&js)),
            "{name}"
        );
    }

    // Flat and derive types: their own name, nothing nested.
    assert_eq!(TestUser::name(), "TestUser");
    assert_eq!(ReqBody::name(), "CreateRequest");
    assert_eq!(DeriveUser::name(), "DeriveUser");
    for collect in [
        <TestUser as ToSchema>::schemas as fn(&mut Vec<(String, RefOr<Schema>)>),
        <DeriveUser as ToSchema>::schemas,
        <ReqBody as ToSchema>::schemas,
    ] {
        let mut nested = Vec::new();
        collect(&mut nested);
        assert!(nested.is_empty());
    }
    let user = &schemas["DeriveUser"];
    assert_eq!(user["properties"]["name"]["minLength"], 2);
    assert_eq!(user["properties"]["name"]["maxLength"], 50);
    assert_eq!(user["properties"]["email"]["format"], "email");
    assert_eq!(required(&schemas["TestUser"]), ["name", "email"]);
    assert_eq!(schemas["CreateRequest"]["type"], "object");
}

/// Given a derive type with `serde(rename_all = "camelCase")`, when it is
/// documented and a client sends a camelCase body, then the document and the
/// parser agree on the wire names; the optional field is not required.
#[test]
fn renamed_fields_are_documented_as_sent() {
    let schema = to_json(&DeriveRenamedRequest::schema());
    let mut keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "emailAddress",
            "firstName",
            "isActive",
            "streetNumber",
            "streetNumberAddition"
        ]
    );
    let mut req = required(&schema);
    req.sort();
    assert_eq!(
        req,
        ["emailAddress", "firstName", "isActive", "streetNumber"]
    );

    let parsed = DeriveRenamedRequest::vld_parse(
        r#"{"firstName": "John", "emailAddress": "john@example.com", "streetNumber": 42, "isActive": true}"#,
    )
    .unwrap();
    assert_eq!(
        (
            parsed.first_name.as_str(),
            parsed.email_address.as_str(),
            parsed.street_number,
            parsed.is_active
        ),
        ("John", "john@example.com", 42, true)
    );
    assert!(parsed.street_number_addition.is_none());
}

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct TicketFilter {
        pub q: String => vld::string().min(1).max(200).describe("search text"),
        pub status: String => vld::string().with_default("open".into()),
        pub assignee: Option<String> => vld::string()
            .email()
            .optional()
            .describe("assignee email")
            .message("bad assignee"),
        pub page: Option<i64> => vld::number().int().gte(1).optional(),
        pub include_closed: Option<bool> => vld::boolean().nullish(),
        pub cursor: Option<String> => vld::string().nullable().describe("opaque cursor"),
    }
}
impl_to_schema!(TicketFilter);

vld::schema! {
    #[derive(Debug)]
    pub struct CreateTicket {
        pub title: String => vld::string()
            .min(3)
            .max(120)
            .describe("ticket title")
            .refine(|s| !s.trim().is_empty(), "blank"),
        pub body: String => vld::string().min(1),
        pub priority: String => vld::string().with_default("normal".into()),
        pub labels: Option<Vec<String>> => vld::array(vld::string().min(1)).optional(),
        pub due_at: Option<String> => vld::string().nullish().describe("ISO date"),
        pub reporter: Option<String> => vld::string()
            .email()
            .nullable()
            .describe("reporter email")
            .message("bad reporter"),
        pub meta: Option<String> => vld::string()
            .optional()
            .describe("freeform")
            .catch(None)
            .transform(|v| v.map(|s| s.to_lowercase())),
    }
}
impl_to_schema!(CreateTicket);

#[derive(Debug, vld::Validate)]
#[allow(dead_code)]
struct DerivePatchTicket {
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
impl_to_schema!(DerivePatchTicket);

/// Snapshot with sorted map keys; utoipa 5 and 6 must produce the same.
macro_rules! snap {
    ($name:expr, $value:expr) => {
        insta::with_settings!({ sort_maps => true }, {
            let value = $value;
            insta::assert_json_snapshot!($name, value);
        })
    };
}

/// Given `schema!` / derive types with optional, nullable, nullish, defaulted,
/// refined, caught and transformed fields, when utoipa documents them as
/// bodies and query parameters, then only plain and nullable fields are
/// required, parameters carry the field descriptions, and the documents match
/// the snapshots.
#[test]
fn the_required_matrix_follows_vld() {
    let js = CreateTicket::json_schema();
    let via_fn = to_json(&json_schema_to_schema(&js));
    assert_eq!(required(&js), ["title", "body", "reporter"]);
    assert_eq!(required(&via_fn), ["title", "body", "reporter"]);
    assert_eq!(via_fn, to_json(&CreateTicket::schema()));

    let derive = to_json(&DerivePatchTicket::schema());
    assert_eq!(required(&derive), ["title", "cursor"]);

    // Parameter order follows the JSON object's key order: sort.
    let mut params = json_schema_to_params(&TicketFilter::json_schema(), ParameterIn::Query);
    assert_eq!(
        to_json(&params),
        to_json(&TicketFilter::into_params(|| None))
    );
    params.sort_by(|a, b| a.name.cmp(&b.name));
    let summary: Vec<(&str, bool, Option<&str>)> = params
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                matches!(p.required, Required::True),
                p.description.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("assignee", false, Some("assignee email")),
            ("cursor", true, Some("opaque cursor")),
            ("include_closed", false, None),
            ("page", false, None),
            ("q", true, Some("search text")),
            ("status", false, None),
        ]
    );
    // A nullable / nullish parameter documents its non-null schema.
    let cursor = to_json(&params[1].schema);
    assert_eq!(cursor["type"], "string", "{cursor}");
    let page = to_json(&params[3].schema);
    assert_eq!(
        (page["type"].clone(), page["minimum"].clone()),
        (json!("integer"), json!(1))
    );

    let renamed = to_json(&DeriveRenamedRequest::schema());
    assert!(!required(&renamed).contains(&"streetNumberAddition"));

    snap!(
        "required_matrix",
        json!({
            "create_ticket": via_fn,
            "derive_patch_ticket": derive,
            "ticket_filter_params": to_json(&params),
            "derive_renamed_request": renamed,
        })
    );
    let full = to_json(&json_schema_to_schema(&TestUser::json_schema()));
    assert_eq!(required(&full), ["name", "email"]);
    snap!("roundtrip_full_struct", full);
}

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct SearchParams {
        pub sample: String => vld::string().min(16).max(200),
        pub page: Option<i64> => vld::number().int().gte(1).optional(),
    }
}
impl_to_schema!(SearchParams);

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Path)]
    pub struct PathParams {
        pub id: i64 => vld::number().int().positive(),
    }
}
impl_to_schema!(PathParams);

vld::schema! {
    #[derive(Debug)]
    pub struct NoLocation {
        pub q: String => vld::string().min(1),
    }
}
impl_to_schema!(NoLocation);

#[derive(Debug, vld::Validate)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct DeriveQuery {
    #[vld(vld::string().min(16))]
    sample: String,
}
impl_to_schema!(DeriveQuery);

macro_rules! located {
    ($($name:ident => $loc:ident),* $(,)?) => {$(
        vld::schema! {
            #[derive(Debug)]
            pub struct $name {
                pub id: i64 => vld::number().int().positive(),
            }
        }
        impl_to_schema!($name, $loc);
    )*};
}
located!(InQuery => query, InPath => path, InHeader => header, InCookie => cookie);

fn location(params: &[vld_utoipa::utoipa::openapi::path::Parameter]) -> Vec<String> {
    params
        .iter()
        .map(|p| to_json(&p.parameter_in).as_str().unwrap().to_string())
        .collect()
}

/// Given parameter structs located by utoipa's attribute, by the macro suffix
/// or not at all, when a route lists them in `params(...)`, then each field
/// is a parameter in the right place with its constraints: the route's own
/// location wins, then the macro suffix, then the attribute; an unlocated
/// struct gives no parameters.
#[test]
fn parameters_land_where_the_route_wants_them() {
    let params = SearchParams::into_params(|| Some(ParameterIn::Query));
    assert_eq!(params.len(), 2);
    let sample = params.iter().find(|p| p.name == "sample").unwrap();
    assert!(matches!(sample.required, Required::True));
    let schema = to_json(&sample.schema);
    assert_eq!(
        (
            schema["type"].clone(),
            schema["minLength"].clone(),
            schema["maxLength"].clone()
        ),
        (json!("string"), json!(16), json!(200))
    );
    let page = params.iter().find(|p| p.name == "page").unwrap();
    assert!(matches!(page.required, Required::False));
    assert_eq!(to_json(&page.schema)["minimum"], 1);

    // The attribute alone.
    assert_eq!(
        location(&SearchParams::into_params(|| None)),
        ["query", "query"]
    );
    let path = PathParams::into_params(|| None);
    assert_eq!(location(&path), ["path"]);
    assert_eq!(path[0].name, "id");
    assert!(matches!(path[0].required, Required::True));
    assert_eq!(to_json(&path[0].schema)["type"], "integer");
    let derive = DeriveQuery::into_params(|| None);
    assert_eq!(location(&derive), ["query"]);
    assert_eq!(to_json(&derive[0].schema)["minLength"], 16);

    // The macro suffix.
    assert_eq!(location(&InQuery::into_params(|| None)), ["query"]);
    assert_eq!(location(&InPath::into_params(|| None)), ["path"]);
    assert_eq!(location(&InHeader::into_params(|| None)), ["header"]);
    assert_eq!(location(&InCookie::into_params(|| None)), ["cookie"]);

    // The route's location wins over both.
    assert_eq!(
        location(&PathParams::into_params(|| Some(ParameterIn::Query))),
        ["query"]
    );
    assert_eq!(
        location(&InQuery::into_params(|| Some(ParameterIn::Header))),
        ["header"]
    );
    assert_eq!(
        location(&NoLocation::into_params(|| Some(ParameterIn::Cookie))),
        ["cookie"]
    );

    // Nowhere to put them: nothing.
    assert!(NoLocation::into_params(|| None).is_empty());
    assert!(json_schema_to_params(&json!("not an object"), ParameterIn::Query).is_empty());
    assert!(json_schema_to_params(&json!({"type": "object"}), ParameterIn::Query).is_empty());

    // Location hints.
    for (hint, want) in [
        ("query", "query"),
        ("path", "path"),
        ("header", "header"),
        ("cookie", "cookie"),
    ] {
        assert_eq!(to_json(&parameter_in_from_str(hint).unwrap()), json!(want));
    }
    assert!(parameter_in_from_str("body").is_none());
    assert!(matches!(
        resolve_parameter_in::<PathParams>(|| None),
        Some(ParameterIn::Path)
    ));
    assert!(matches!(
        resolve_parameter_in::<PathParams>(|| Some(ParameterIn::Header)),
        Some(ParameterIn::Header)
    ));
    assert!(resolve_parameter_in::<NoLocation>(|| None).is_none());
}

/// Given hand-written parameter schemas with nullable unions carrying the
/// description and default outside, when they become parameters, then the
/// parameter takes the description and the schema the default.
#[test]
fn nullable_parameters_keep_outer_annotations() {
    let schema = json!({
        "type": "object",
        "properties": {
            "page": {
                "anyOf": [{"type": "integer", "minimum": 1}, {"type": "null"}],
                "description": "page number",
                "default": 1
            },
            "q": {"oneOf": [{"type": "string"}, {"type": "null"}], "description": "query"},
            "both": {"anyOf": [{"type": "string"}, {"type": "integer"}]},
            "single": {"anyOf": [{"type": "string", "minLength": 1}]},
            "plain": true
        },
        "required": ["q", 7]
    });
    let params = to_json(&json_schema_to_params(&schema, ParameterIn::Query));
    let find = |n: &str| {
        params
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == n)
            .unwrap()
            .clone()
    };
    let page = find("page");
    assert_eq!(page["description"], "page number");
    assert_eq!(page["required"], false);
    assert_eq!(page["schema"]["type"], "integer");
    assert_eq!(page["schema"]["minimum"], 1);
    assert_eq!(page["schema"]["default"], 1);
    let q = find("q");
    assert_eq!(
        (q["description"].clone(), q["required"].clone()),
        (json!("query"), json!(true))
    );
    assert_eq!(q["schema"]["type"], "string");
    assert_eq!(find("both")["schema"]["anyOf"].as_array().unwrap().len(), 2);
    assert!(find("plain")["schema"].get("type").is_none());
    // A one-branch union without `null` is not unwrapped.
    assert_eq!(find("single")["schema"]["anyOf"][0]["minLength"], 1);
}
