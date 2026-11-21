
// A request body.
vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}
impl_to_schema!(User);

// Query / path parameters: utoipa's own attribute picks the location.
vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct Search {
        pub q: String => vld::string().min(1).max(200),
    }
}
impl_to_schema!(Search);

// A custom component name.
vld::schema! {
    #[derive(Debug)]
    pub struct Req { pub x: String => vld::string() }
}
impl_to_schema!(Req, "CreateUserRequest");

// `User` can now be used in `#[utoipa::path(request_body = User)]`,
// `Search` in `params(Search)`.
use utoipa::{IntoParams, ToSchema};
assert_eq!(<Req as ToSchema>::name(), "CreateUserRequest");
assert_eq!(Search::into_params(|| None).len(), 1);

// Any JSON Schema value: `utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>`.
let schema = json_schema_to_schema(&serde_json::json!({
    "type": "object",
    "required": ["name"],
    "properties": { "name": { "type": "string", "minLength": 1 } }
}));
assert!(matches!(schema, utoipa::openapi::RefOr::T(_)));
