//! Regression tests for the live end-to-end findings (0.5.0).

use rocket::http::{ContentType, Header, Status};
use rocket::local::blocking::Client;
use rocket::routes;
use vld_rocket::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Search {
        pub q: String => vld::string(),
        pub age: i64 => vld::number().int(),
        pub zip: String => vld::string(),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

#[rocket::post("/j", data = "<body>")]
fn json(body: VldJson<Named>) -> String {
    body.name.clone()
}

#[rocket::get("/q")]
fn query(q: VldQuery<Search>) -> String {
    format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(","))
}

fn client() -> Client {
    let rocket = rocket::build()
        .mount("/", routes![json, query])
        .register("/", vld_catchers());
    Client::tracked(rocket).expect("valid rocket")
}

#[test]
fn json_content_types() {
    let c = client();
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        let resp = c
            .post("/j")
            .header(Header::new("Content-Type", ct))
            .body(r#"{"name":"alice"}"#)
            .dispatch();
        assert_eq!(resp.status(), Status::Ok, "{ct}");
        assert_eq!(resp.into_string().unwrap(), "alice");
    }
    for ct in [None, Some(ContentType::Plain)] {
        let mut req = c.post("/j").body(r#"{"name":"alice"}"#);
        if let Some(ct) = ct.clone() {
            req = req.header(ct);
        }
        let resp = req.dispatch();
        assert_eq!(resp.status(), Status::UnsupportedMediaType, "{ct:?}");
        let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[test]
fn validation_issues_carry_code() {
    let c = client();
    let resp = c
        .post("/j")
        .header(ContentType::JSON)
        .body(r#"{"name":"a"}"#)
        .dispatch();
    assert_eq!(resp.status(), Status::UnprocessableEntity);
    let body: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[test]
fn query_coercion_follows_schema() {
    let c = client();
    let resp = c.get("/q?q=123&age=30&zip=01234&tags=a").dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.into_string().unwrap(), "123|30|01234|a");
}
