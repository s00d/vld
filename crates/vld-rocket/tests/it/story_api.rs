//! Stories of a Rocket API using every `vld-rocket` guard and the JSON
//! catchers: one journey per test, each request through a local client.

use rocket::http::{ContentType, Header, Status};
use rocket::local::blocking::{Client, LocalRequest};
use rocket::{catchers, routes};
use serde_json::{Value, json};
use vld_rocket::prelude::*;

struct Reply {
    status: Status,
    content_type: Option<ContentType>,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("{e}: {}", self.text))
    }

    /// `(path, code)` of a 422 body; checks the body shape on the way.
    #[track_caller]
    fn issues(&self) -> Vec<(String, String)> {
        assert_eq!(self.status, Status::UnprocessableEntity, "{}", self.text);
        assert_eq!(self.content_type, Some(ContentType::JSON));
        let body = self.json();
        assert_eq!(body["error"], "Validation failed");
        body["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| {
                assert!(i["message"].is_string(), "{i}");
                (
                    i["path"].as_str().unwrap().into(),
                    i["code"].as_str().unwrap().into(),
                )
            })
            .collect()
    }

    #[track_caller]
    fn ok(&self) -> &str {
        assert_eq!(self.status, Status::Ok, "{}", self.text);
        &self.text
    }

    #[track_caller]
    fn too_large(&self) {
        assert_eq!(self.status, Status::PayloadTooLarge, "{}", self.text);
        assert_eq!(self.json(), json!({"error": "Payload too large"}));
    }
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

fn send(req: LocalRequest<'_>) -> Reply {
    let resp = req.dispatch();
    Reply {
        status: resp.status(),
        content_type: resp.content_type(),
        text: resp.into_string().unwrap_or_default(),
    }
}

fn get<'c>(c: &'c Client, uri: &str, headers: &[(&'static str, &'static str)]) -> LocalRequest<'c> {
    let mut req = c.get(uri.to_string());
    for (k, v) in headers {
        req.add_header(Header::new(*k, *v));
    }
    req
}

fn post<'c>(c: &'c Client, uri: &str, ct: ContentType, body: impl AsRef<[u8]>) -> LocalRequest<'c> {
    c.post(uri.to_string()).header(ct).body(body)
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<NewUser>) -> rocket::serde::json::Json<Value> {
    rocket::serde::json::Json(json!({"name": user.name, "email": user.email}))
}

#[rocket::post("/json", data = "<body>")]
fn json_name(body: VldJson<Named>) -> String {
    body.name.clone()
}

#[rocket::post("/form", data = "<body>")]
fn form_name(body: VldForm<Named>) -> String {
    body.name.clone()
}

fn client(rocket: rocket::Rocket<rocket::Build>) -> Client {
    Client::tracked(rocket).expect("valid rocket")
}

/// Given a user API mounted at `/api/v1` with the vld catchers,
/// when clients send valid bodies, schema errors, malformed JSON, wrong
/// media types and non-UTF-8 forms,
/// then valid bodies reach the route and every failure is the documented
/// JSON error from the catchers: 422 with `(path, code)` issues, 415.
#[test]
fn bodies_create_users_under_a_mount_point() {
    let c = client(
        rocket::build()
            .mount("/api/v1", routes![create_user, json_name, form_name])
            .register("/", vld_catchers()),
    );

    let r = send(post(
        &c,
        "/api/v1/users",
        ContentType::JSON,
        r#"{"name":"Alice","email":"alice@example.com"}"#,
    ));
    assert_eq!(r.ok(), r#"{"email":"alice@example.com","name":"Alice"}"#);
    let r = send(post(
        &c,
        "/api/v1/users",
        ContentType::JSON,
        r#"{"name":"A","email":"not-email"}"#,
    ));
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let r = send(post(&c, "/api/v1/users", ContentType::JSON, "{}"));
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".email", "missing_field")])
    );
    let r = send(post(&c, "/api/v1/users", ContentType::JSON, "[1]"));
    assert_eq!(r.issues(), issues(&[("", "invalid_type")]));
    let r = send(post(&c, "/api/v1/users", ContentType::JSON, "not json"));
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send(post(&c, "/api/v1/users", ContentType::Plain, "{}"));
    assert_eq!(r.status, Status::UnsupportedMediaType);
    assert_eq!(
        r.json(),
        json!({"error": "Unsupported Media Type", "message": vld_http_common::UNSUPPORTED_MEDIA_TYPE_MESSAGE})
    );
    let r = send(post(&c, "/api/v1/form", ContentType::JSON, "name=Al"));
    assert_eq!(
        r.json()["message"],
        vld_http_common::UNSUPPORTED_FORM_MEDIA_TYPE_MESSAGE
    );

    assert_eq!(
        send(post(&c, "/api/v1/form", ContentType::Form, "name=Al+B")).ok(),
        "Al B"
    );
    let r = send(post(&c, "/api/v1/form", ContentType::Form, "name=A"));
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));
    let r = send(post(
        &c,
        "/api/v1/form",
        ContentType::Form,
        b"name=\xFF\xFE",
    ));
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
}

fn limited(form: u64, json: u64) -> Client {
    let figment = rocket::Config::figment()
        .merge(("limits.form", form))
        .merge(("limits.json", json));
    client(
        rocket::custom(figment)
            .mount("/", routes![json_name, form_name])
            .register("/", vld_catchers()),
    )
}

/// Given Rocket `limits` for `json` and `form`,
/// when they are set low, left at Rocket's built-ins or set to exactly the
/// built-in values,
/// then a body above a set limit is 413, a body at the limit passes, and
/// while a limit is at Rocket's built-in value the shared 2 MiB default
/// applies instead.
#[test]
fn body_limits_come_from_rocket_config() {
    let c = limited(16, 16);
    send(post(
        &c,
        "/json",
        ContentType::JSON,
        r#"{"name":"a-long-name"}"#,
    ))
    .too_large();
    send(post(
        &c,
        "/form",
        ContentType::Form,
        "name=a-very-long-name",
    ))
    .too_large();
    assert_eq!(
        send(post(&c, "/json", ContentType::JSON, r#"{"name":"12345"}"#)).ok(),
        "12345"
    );
    assert_eq!(
        send(post(&c, "/form", ContentType::Form, "name=12345678901")).ok(),
        "12345678901"
    );

    // Built-in limits (json 1 MiB, form 32 KiB): 2 MiB applies.
    let name = "x".repeat(1536 * 1024);
    for c in [
        client(
            rocket::build()
                .mount("/", routes![json_name, form_name])
                .register("/", vld_catchers()),
        ),
        limited(32 * 1024, 1024 * 1024),
    ] {
        assert_eq!(
            send(post(
                &c,
                "/json",
                ContentType::JSON,
                format!(r#"{{"name":"{name}"}}"#)
            ))
            .ok()
            .len(),
            name.len()
        );
        assert_eq!(
            send(post(&c, "/form", ContentType::Form, format!("name={name}")))
                .ok()
                .len(),
            name.len()
        );
        let big = "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT);
        send(post(
            &c,
            "/json",
            ContentType::JSON,
            format!(r#"{{"name":"{big}"}}"#),
        ))
        .too_large();
        send(post(&c, "/form", ContentType::Form, format!("name={big}"))).too_large();
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Search {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
        pub active: Option<bool> => vld::boolean().optional(),
        pub tag: Option<Vec<String>> => vld::array(vld::string()).optional(),
    }
}

#[rocket::get("/search")]
fn search(p: VldQuery<Search>) -> String {
    format!(
        "q={} page={:?} limit={:?} active={:?} tag={:?}",
        p.q, p.page, p.limit, p.active, p.tag
    )
}

/// Given a `GET /search` route reading `VldQuery<Search>`,
/// when clients send full, partial, encoded and invalid query strings,
/// then values are decoded and coerced by the schema and a bad value is a
/// 422 issue at its key.
#[test]
fn query_strings_drive_a_search() {
    let c = client(
        rocket::build()
            .mount("/", routes![search])
            .register("/", vld_catchers()),
    );

    let r = send(get(&c, "/search?q=rust&page=2&limit=25", &[]));
    assert_eq!(
        r.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None"
    );
    let r = send(get(&c, "/search?q=hello+world&active=false", &[]));
    assert_eq!(
        r.ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None"
    );
    let r = send(get(
        &c,
        "/search?q=01234&active=true&tag=a&tag=caf%C3%A9",
        &[],
    ));
    assert_eq!(
        r.ok(),
        r#"q=01234 page=None limit=None active=Some(true) tag=Some(["a", "café"])"#
    );

    let r = send(get(&c, "/search?page=0&limit=200", &[]));
    assert_eq!(
        r.issues(),
        issues(&[
            (".q", "missing_field"),
            (".page", "too_small"),
            (".limit", "too_big")
        ])
    );
    assert_eq!(
        send(get(&c, "/search", &[])).issues(),
        issues(&[(".q", "missing_field")])
    );
}

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
    pub struct Owner {
        pub owner: String => vld::string(),
        pub id: i64 => vld::number().int(),
    }
}

#[rocket::get("/owners/<owner>/static/<id>")]
fn owned(owner: &str, id: &str, p: VldPath<Owner>) -> String {
    let _ = (owner, id);
    format!("{}#{}", p.owner, p.id)
}

/// Given routes with a trailing `<rest..>` segment and a static segment
/// between parameters, mounted at `/api/v1`,
/// when clients request valid, encoded and invalid paths,
/// then `VldPath` maps each `<param>` to its segment (skipping the mount
/// base and static segments, joining the rest with `/`) and a bad id is a
/// 422 issue at `.id`.
#[test]
fn path_segments_address_files() {
    let c = client(
        rocket::build()
            .mount("/api/v1", routes![item_file, owned])
            .register("/", vld_catchers()),
    );

    assert_eq!(
        send(get(&c, "/api/v1/items/42/files/a/b%20c.txt", &[])).ok(),
        "42:a/b c.txt"
    );
    assert_eq!(send(get(&c, "/api/v1/items/7/files/x", &[])).ok(), "7:x");
    assert_eq!(
        send(get(&c, "/api/v1/items/0/files/x", &[])).issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(get(&c, "/api/v1/items/abc/files/x", &[])).issues(),
        issues(&[(".id", "invalid_type")])
    );
    assert_eq!(
        send(get(&c, "/api/v1/owners/ann/static/3", &[])).ok(),
        "ann#3"
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
        pub x_tag: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
        pub dark_mode: Option<bool> => vld::boolean().optional(),
    }
}

#[rocket::get("/protected")]
fn protected(h: VldHeaders<AuthHeaders>) -> String {
    format!(
        "auth={} rid={:?} tag={:?}",
        h.authorization, h.x_request_id, h.x_tag
    )
}

#[rocket::get("/dashboard")]
fn dashboard(c: VldCookie<Session>) -> String {
    format!(
        "sid={} theme={:?} dark={:?}",
        c.session_id, c.theme, c.dark_mode
    )
}

/// Given a dashboard behind `VldHeaders<AuthHeaders>` and
/// `VldCookie<Session>`,
/// when clients send complete, partial, repeated and missing credentials,
/// then header names map to snake_case fields, repeats join with `", "`,
/// cookies are unquoted, decoded and coerced (from every `Cookie` header),
/// and a missing required value is a 422 `missing_field`.
#[test]
fn headers_and_cookies_open_a_dashboard() {
    let c = client(
        rocket::build()
            .mount("/", routes![protected, dashboard])
            .register("/", vld_catchers()),
    );

    let r = send(get(
        &c,
        "/protected",
        &[
            ("Authorization", "Bearer token123"),
            ("X-Request-Id", "abc-123"),
        ],
    ));
    assert_eq!(
        r.ok(),
        r#"auth=Bearer token123 rid=Some("abc-123") tag=None"#
    );
    let r = send(get(
        &c,
        "/protected",
        &[("authorization", "t"), ("x-tag", "a"), ("x-tag", "café")],
    ));
    assert_eq!(r.ok(), r#"auth=t rid=None tag=Some("a, café")"#);
    assert_eq!(
        send(get(&c, "/protected", &[])).issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = send(get(
        &c,
        "/dashboard",
        &[
            ("Cookie", "session_id=abc123; theme=dark"),
            ("Cookie", "dark_mode=true"),
        ],
    ));
    assert_eq!(r.ok(), r#"sid=abc123 theme=Some("dark") dark=Some(true)"#);
    let r = send(get(
        &c,
        "/dashboard",
        &[("Cookie", "session_id=\"abc%20def\"")],
    ));
    assert_eq!(r.ok(), "sid=abc def theme=None dark=None");
    assert_eq!(
        send(get(&c, "/dashboard", &[])).issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct N {
        pub n: i64 => vld::number().int().min(1),
    }
}

#[rocket::post("/mixed", data = "<body>")]
fn mixed(q: Option<VldQuery<N>>, body: rocket::serde::json::Json<Value>) -> String {
    let _ = (q, body);
    "ok".into()
}

#[rocket::post("/both", data = "<body>")]
fn both(q: Option<VldQuery<N>>, body: VldJson<Named>) -> String {
    format!("{}|{}", q.is_some(), body.name)
}

/// Given routes where an optional vld guard fails before another guard,
/// when the later guard fails too, or Rocket's own `Json` rejects the body,
/// then the catcher answers for the last vld failure, Rocket's own 400 gets
/// a generic `{"error": "Bad Request"}` (never a stale vld body), and an
/// app registering only some catchers still gets JSON for those.
#[test]
fn catchers_answer_for_the_failure_that_set_the_status() {
    let c = client(
        rocket::build()
            .mount("/", routes![mixed, both])
            .register("/", vld_catchers()),
    );

    let r = send(post(&c, "/mixed?n=0", ContentType::JSON, "{bad"));
    assert_eq!(
        (r.status, r.json()),
        (Status::BadRequest, json!({"error": "Bad Request"}))
    );
    assert_eq!(
        send(post(&c, "/mixed?n=0", ContentType::JSON, "{}")).ok(),
        "ok"
    );

    assert_eq!(
        send(post(&c, "/both?n=0", ContentType::JSON, r#"{"name":"Al"}"#)).ok(),
        "false|Al"
    );
    assert_eq!(
        send(post(&c, "/both?n=1", ContentType::JSON, r#"{"name":"Al"}"#)).ok(),
        "true|Al"
    );
    let r = send(post(&c, "/both?n=0", ContentType::JSON, r#"{"name":"A"}"#));
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));

    // Only the 422 and 400 catchers registered.
    let c = client(
        rocket::build()
            .mount("/", routes![create_user, search])
            .register("/", catchers![vld_422_catcher, vld_400_catcher]),
    );
    let r = send(post(
        &c,
        "/users",
        ContentType::JSON,
        r#"{"name":"A","email":"a@b.co"}"#,
    ));
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));
    assert_eq!(
        send(get(&c, "/search?q=", &[])).issues(),
        issues(&[(".q", "too_small")])
    );

    // Registered one by one, the 413 and 415 catchers answer as in the set.
    let c = client(
        rocket::custom(rocket::Config::figment().merge(("limits.json", 8)))
            .mount("/", routes![json_name])
            .register("/", catchers![vld_413_catcher, vld_415_catcher]),
    );
    send(post(
        &c,
        "/json",
        ContentType::JSON,
        r#"{"name":"long name"}"#,
    ))
    .too_large();
    let r = send(post(&c, "/json", ContentType::Plain, "{}"));
    assert_eq!(
        (r.status, r.json()["error"].clone()),
        (
            Status::UnsupportedMediaType,
            json!("Unsupported Media Type")
        )
    );
}
