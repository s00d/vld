use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;
use rocket::{catchers, routes};
use vld_rocket::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Pagination {
        pub page: i64  => vld::number().int().min(1),
        pub limit: i64 => vld::number().int().min(1).max(100),
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUser>) -> rocket::serde::json::Json<serde_json::Value> {
    rocket::serde::json::Json(serde_json::json!({
        "name": user.name,
        "email": user.email,
    }))
}

#[rocket::get("/items")]
fn list_items(q: VldQuery<Pagination>) -> rocket::serde::json::Json<serde_json::Value> {
    rocket::serde::json::Json(serde_json::json!({
        "page": q.page,
        "limit": q.limit,
    }))
}

fn rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .mount("/", routes![create_user, list_items])
        .register("/", catchers![vld_422_catcher, vld_400_catcher])
}

// ---------------------------------------------------------------------------
// Tests — JSON body
// ---------------------------------------------------------------------------

#[test]
fn json_valid() {
    let client = Client::tracked(rocket()).expect("valid rocket");
    let resp = client
        .post("/users")
        .header(ContentType::JSON)
        .body(r#"{"name": "Alice", "email": "alice@example.com"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert_eq!(body["name"], "Alice");
}

#[test]
fn json_invalid_validation() {
    let client = Client::tracked(rocket()).expect("valid rocket");
    let resp = client
        .post("/users")
        .header(ContentType::JSON)
        .body(r#"{"name": "A", "email": "not-email"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert_eq!(body["error"], "Validation failed");
    assert!(body["issues"].as_array().unwrap().len() >= 2);
}

#[test]
fn json_malformed() {
    let client = Client::tracked(rocket()).expect("valid rocket");
    let resp = client
        .post("/users")
        .header(ContentType::JSON)
        .body("not json")
        .dispatch();
    assert!(resp.status() == Status::BadRequest || resp.status() == Status::UnprocessableEntity);
}

// ---------------------------------------------------------------------------
// Tests — Query
// ---------------------------------------------------------------------------

#[test]
fn query_valid() {
    let client = Client::tracked(rocket()).expect("valid rocket");
    let resp = client.get("/items?page=2&limit=25").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert_eq!(body["page"], 2);
    assert_eq!(body["limit"], 25);
}

#[test]
fn query_invalid() {
    let client = Client::tracked(rocket()).expect("valid rocket");
    let resp = client.get("/items?page=0&limit=200").dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert_eq!(body["error"], "Validation failed");
}
