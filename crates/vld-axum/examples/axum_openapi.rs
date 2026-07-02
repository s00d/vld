//! Axum runtime extractors + vld-utoipa OpenAPI (query constraints in spec).
//!
//! Run:
//! ```sh
//! cargo run -p vld-axum --example axum_openapi
//! ```

use utoipa::IntoParams;
use vld_axum::prelude::VldSchema;
use vld_axum::VldQuery;
use vld_utoipa::impl_to_schema;

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct SearchParams {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().gte(1).optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

impl_to_schema!(SearchParams);
impl_to_schema!(CreateUser);

#[utoipa::path(
    get,
    path = "/search",
    params(SearchParams),
    responses((status = 200, description = "Search results")),
)]
#[allow(dead_code)]
async fn search(_params: VldQuery<SearchParams>) {}

#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUser,
    responses(
        (status = 200, description = "User created"),
        (status = 422, description = "Validation failed"),
    ),
)]
#[allow(dead_code)]
async fn create_user() {}

#[derive(utoipa::OpenApi)]
#[openapi(paths(search, create_user), components(schemas(CreateUser)))]
struct ApiDoc;

fn main() {
    use utoipa::OpenApi;

    let spec = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec).unwrap();
    println!("{json}");

    let params = SearchParams::into_params(|| None);
    assert_eq!(params.len(), 2);
    let q = params.iter().find(|p| p.name == "q").unwrap();
    let q_schema = serde_json::to_value(q.schema.as_ref().unwrap()).unwrap();
    assert_eq!(q_schema["minLength"], 1);
    assert_eq!(q_schema["maxLength"], 200);
    println!("\nSearchParams OpenAPI field `q` includes vld minLength/maxLength.");
}
