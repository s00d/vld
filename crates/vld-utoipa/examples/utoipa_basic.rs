use vld::prelude::*;
use vld_utoipa::impl_to_schema;

// Nested struct — automatically registered as OpenAPI component
vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct Address {
        pub city: String => vld::string().min(1).max(100),
        pub zip: String => vld::string().min(5).max(10),
    }
}

impl_to_schema!(Address);

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
        pub address: Address => vld::nested!(Address),
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

impl_to_schema!(CreateUser);
impl_to_schema!(UserResponse);

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

// Address is auto-registered via ToSchema::schemas() — no need to list it manually!
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(create_user, get_user),
    components(schemas(CreateUser, UserResponse))
)]
struct ApiDoc;

fn main() {
    use utoipa::OpenApi;

    let spec = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec).unwrap();
    println!("{json}");

    println!("\n=== Validation ===");
    let input = r#"{
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30,
        "address": {"city": "Berlin", "zip": "10115"}
    }"#;
    match CreateUser::parse(input) {
        Ok(u) => println!("valid: {:?}", u),
        Err(e) => println!("errors: {e}"),
    }

    match CreateUser::parse(r#"{"name": "A", "email": "bad", "address": {"city": "", "zip": "1"}}"#) {
        Ok(_) => println!("valid"),
        Err(e) => println!("errors:\n{e}"),
    }
}
