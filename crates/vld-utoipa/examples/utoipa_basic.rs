use vld::prelude::*;
use vld_utoipa::impl_to_schema;

// Define validated structs with vld
vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserResponse {
        pub id: i64 => vld::number().int().positive(),
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

// Bridge to utoipa — single source of truth
impl_to_schema!(CreateUser);
impl_to_schema!(UserResponse);

// API handlers (needed for utoipa path registration)
#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUser,
    responses(
        (status = 200, description = "User created", body = UserResponse),
        (status = 422, description = "Validation failed"),
    )
)]
fn create_user() {}

#[allow(dead_code)]
#[utoipa::path(
    get,
    path = "/users/{id}",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 404, description = "Not found"),
    )
)]
fn get_user() {}

// Build the full OpenAPI spec
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(create_user, get_user),
    components(schemas(CreateUser, UserResponse))
)]
struct ApiDoc;

fn main() {
    use utoipa::OpenApi;

    // Full OpenAPI spec as JSON
    let spec = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec).unwrap();
    println!("{json}");

    // Validation still works as usual
    println!("\n=== Validation ===");
    match CreateUser::parse(r#"{"name": "A", "email": "bad"}"#) {
        Ok(_) => println!("valid"),
        Err(e) => println!("errors: {e}"),
    }

    match CreateUser::parse(r#"{"name": "Alice", "email": "alice@example.com", "age": 30}"#) {
        Ok(u) => println!("valid: {:?}", u),
        Err(e) => println!("errors: {e}"),
    }
}
