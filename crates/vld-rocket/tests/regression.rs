//! Regression tests for the 0.5.0 audit fixes.

use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;
use rocket::routes;
use vld_rocket::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct ItemPath {
        pub id: i64 => vld::number().int().min(1),
        pub rest: String => vld::string(),
    }
}

#[rocket::get("/items/<id>/files/<rest..>")]
fn item_file(id: &str, rest: std::path::PathBuf, p: VldPath<ItemPath>) -> String {
    let _ = (id, rest);
    format!("{}:{}", p.id, p.rest)
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

#[rocket::post("/json", data = "<body>")]
fn json(body: VldJson<Named>) -> String {
    body.name.clone()
}

#[rocket::post("/form", data = "<body>")]
fn form(body: VldForm<Named>) -> String {
    body.name.clone()
}

fn client(form_limit: u64, json_limit: u64) -> Client {
    let figment = rocket::Config::figment()
        .merge(("limits.form", form_limit))
        .merge(("limits.json", json_limit));
    let rocket = rocket::custom(figment)
        .mount("/api/v1", routes![item_file, json, form])
        .register("/", vld_catchers());
    Client::tracked(rocket).expect("valid rocket")
}

fn json_body(resp: rocket::local::blocking::LocalResponse<'_>) -> serde_json::Value {
    serde_json::from_str(&resp.into_string().unwrap()).unwrap()
}

#[test]
fn path_params_under_non_root_mount() {
    let c = client(1024, 1024);
    let resp = c.get("/api/v1/items/42/files/a/b%20c.txt").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.into_string().unwrap(), "42:a/b c.txt");
}

#[test]
fn path_validation_error_under_mount() {
    let c = client(1024, 1024);
    let resp = c.get("/api/v1/items/0/files/x").dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
}

#[test]
fn form_over_limit_is_413_json() {
    let c = client(8, 1024);
    let resp = c
        .post("/api/v1/form")
        .header(ContentType::Form)
        .body("name=a-very-long-name")
        .dispatch();
    assert_eq!(resp.status(), Status::PayloadTooLarge);
    assert_eq!(json_body(resp)["error"], "Payload too large");
}

#[test]
fn json_over_limit_is_413_json() {
    let c = client(1024, 8);
    let resp = c
        .post("/api/v1/json")
        .header(ContentType::JSON)
        .body(r#"{"name":"a-very-long-name"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::PayloadTooLarge);
    assert_eq!(json_body(resp)["error"], "Payload too large");
}

#[test]
fn malformed_json_is_422_validation_shape() {
    let c = client(1024, 1024);
    let resp = c
        .post("/api/v1/json")
        .header(ContentType::JSON)
        .body("{")
        .dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
    assert_eq!(json_body(resp)["error"], "Validation failed");
}

#[test]
fn non_utf8_form_is_422() {
    let c = client(1024, 1024);
    let resp = c
        .post("/api/v1/form")
        .header(ContentType::Form)
        .body(b"name=\xFF\xFE".as_slice())
        .dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
    assert_eq!(json_body(resp)["error"], "Validation failed");
}

#[test]
fn form_within_limit_is_ok() {
    let c = client(1024, 1024);
    let resp = c
        .post("/api/v1/form")
        .header(ContentType::Form)
        .body("name=Al+B")
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.into_string().unwrap(), "Al B");
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct N {
        pub n: i64 => vld::number().int().min(1),
    }
}

#[rocket::post("/mixed", data = "<body>")]
fn mixed(q: Option<VldQuery<N>>, body: rocket::serde::json::Json<serde_json::Value>) -> String {
    let _ = (q, body);
    "ok".into()
}

/// The catcher must not answer Rocket's own 400 (malformed `Json`) with the
/// vld error an earlier optional guard stored for a 422.
#[test]
fn catcher_does_not_return_stale_vld_error() {
    let rocket = rocket::build()
        .mount("/", routes![mixed])
        .register("/", vld_catchers());
    let c = Client::tracked(rocket).expect("valid rocket");
    let resp = c
        .post("/mixed?n=0")
        .header(ContentType::JSON)
        .body("{bad")
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
    assert_eq!(json_body(resp), serde_json::json!({"error": "Bad Request"}));
}

/// With Rocket's built-in limits (json 1 MiB, form 32 KiB) left alone, the
/// shared 2 MiB default applies.
#[test]
fn default_limits_are_2_mib() {
    let rocket = rocket::build()
        .mount("/", routes![json, form])
        .register("/", vld_catchers());
    let c = Client::tracked(rocket).expect("valid rocket");
    let name = "x".repeat(1536 * 1024);
    let resp = c
        .post("/json")
        .header(ContentType::JSON)
        .body(format!(r#"{{"name":"{name}"}}"#))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let resp = c
        .post("/form")
        .header(ContentType::Form)
        .body(format!("name={name}"))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}
