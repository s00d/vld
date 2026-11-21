//! Stories of an actix-web API using every `vld-actix` extractor: one
//! journey per test, each request through a real `App` service.

use actix_web::dev::ServiceResponse;
use actix_web::http::StatusCode;
use actix_web::http::header::HeaderValue;
use actix_web::{App, HttpResponse, test, web};
use serde_json::{Value, json};
use vld_web::actix::prelude::*;

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("{e}: {}", self.text))
    }

    /// `(path, code)` of a 422 body; checks the body shape on the way.
    #[track_caller]
    fn issues(&self) -> Vec<(String, String)> {
        assert_eq!(
            self.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{}",
            self.text
        );
        assert_eq!(self.content_type.as_deref(), Some("application/json"));
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
        assert_eq!(self.status, StatusCode::OK, "{}", self.text);
        &self.text
    }

    #[track_caller]
    fn too_large(&self) {
        assert_eq!(self.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", self.text);
        assert_eq!(self.content_type.as_deref(), Some("application/json"));
        assert_eq!(self.json(), json!({"error": "Payload too large"}));
    }
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// Send a `TestRequest` through an initialized service.
macro_rules! send {
    ($app:expr, $req:expr) => {
        reply(test::call_service($app, $req.to_request()).await).await
    };
}

async fn reply(resp: ServiceResponse) -> Reply {
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    let text = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    Reply {
        status,
        content_type,
        text,
    }
}

fn get(uri: &str, headers: &[(&str, &[u8])]) -> test::TestRequest {
    let mut req = test::TestRequest::get().uri(uri);
    for (k, v) in headers {
        req = req.append_header((*k, HeaderValue::from_bytes(v).unwrap()));
    }
    req
}

fn post(
    uri: &str,
    content_type: &str,
    body: impl Into<actix_web::web::Bytes>,
) -> test::TestRequest {
    test::TestRequest::post()
        .uri(uri)
        .insert_header(("content-type", content_type))
        .set_payload(body.into())
}

const JSON: &str = "application/json";
const FORM: &str = "application/x-www-form-urlencoded";

vld::schema! {
    #[derive(Debug)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(50),
        pub age: i64 => vld::number().int().min(0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Login {
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Note {
        pub text: String => vld::string(),
    }
}

async fn create(user: VldJson<NewUser>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{}:{}", user.name, user.age))
}

async fn login(form: VldForm<Login>) -> HttpResponse {
    HttpResponse::Ok().body(format!("user={}", form.username))
}

async fn note(n: VldJson<Note>) -> HttpResponse {
    HttpResponse::Ok().body(n.text.len().to_string())
}

/// Given a user API with a JSON `POST /users` and a form `POST /login`,
/// when clients send valid bodies, schema errors, wrong JSON and
/// non-JSON media types,
/// then valid bodies reach the handler (fields through `Deref`) and every
/// failure is the documented JSON error: 422 with `(path, code)` issues,
/// 415.
#[actix_rt::test]
async fn bodies_create_users_and_log_them_in() {
    let app = test::init_service(
        App::new()
            .route("/users", web::post().to(create))
            .route("/login", web::post().to(login)),
    )
    .await;

    assert_eq!(
        send!(&app, post("/users", JSON, r#"{"name":"Alice","age":25}"#)).ok(),
        "Alice:25"
    );
    let r = send!(&app, post("/users", JSON, r#"{"name":"A","age":-1}"#));
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".age", "too_small")])
    );
    let r = send!(&app, post("/users", JSON, "{}"));
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".age", "missing_field")])
    );
    let r = send!(&app, post("/users", JSON, "[1,2,3]"));
    assert_eq!(r.issues(), issues(&[("", "invalid_type")]));
    let r = send!(&app, post("/users", JSON, "not json"));
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send!(&app, post("/users", "text/plain", "{}"));
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(r.json()["error"], "Unsupported Media Type");
    let r = send!(&app, post("/login", JSON, "username=alice"));
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    assert_eq!(
        send!(
            &app,
            post("/login", FORM, "username=alice&password=secret1234")
        )
        .ok(),
        "user=alice"
    );
    let r = send!(&app, post("/login", FORM, "username=al&password=123"));
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".password", "too_small")])
    );
    let r = send!(&app, post("/login", FORM, ""));
    assert_eq!(
        r.issues(),
        issues(&[
            (".username", "missing_field"),
            (".password", "missing_field")
        ])
    );
}

/// Given the same API with `VldBodyLimit` app data,
/// when the limit is lowered to 8 bytes or raised to 4 MiB,
/// then bodies above it are 413 with the exact JSON body (JSON and form),
/// and a 3 MiB body passes the raised limit (it is not actix's `JsonConfig`,
/// so `application/json;charset` is fine too); at exactly the limit a body
/// passes.
#[actix_rt::test]
async fn the_body_limit_is_the_app_data() {
    let small = test::init_service(
        App::new()
            .app_data(VldBodyLimit(16))
            .route("/notes", web::post().to(note))
            .route("/login", web::post().to(login)),
    )
    .await;
    send!(&small, post("/notes", JSON, r#"{"text":"long enough"}"#)).too_large();
    send!(&small, post("/login", FORM, "username=alice-the-long")).too_large();
    // 16 bytes exactly.
    assert_eq!(
        send!(&small, post("/notes", JSON, r#"{"text":"12345"}"#)).ok(),
        "5"
    );

    let big = test::init_service(
        App::new()
            .app_data(VldBodyLimit(4 * 1024 * 1024))
            .route("/notes", web::post().to(note)),
    )
    .await;
    let text = "x".repeat(3 * 1024 * 1024);
    let r = send!(
        &big,
        post(
            "/notes",
            "application/json;charset",
            format!(r#"{{"text":"{text}"}}"#)
        )
    );
    assert_eq!(r.ok(), (3 * 1024 * 1024).to_string());
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
        pub active: Option<bool> => vld::boolean().optional(),
        pub tag: Option<Vec<String>> => vld::array(vld::string()).optional(),
    }
}

async fn search(p: VldQuery<Search>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "q={} page={:?} limit={:?} active={:?} tag={:?}",
        p.q, p.page, p.limit, p.active, p.tag
    ))
}

/// Given a `GET /search` handler reading `VldQuery<Search>`,
/// when clients send full, partial, encoded and invalid query strings,
/// then values are decoded and coerced by the schema (numbers, booleans,
/// repeated keys, non-canonical numbers kept as text) and a bad value is a
/// 422 issue at its key.
#[actix_rt::test]
async fn query_strings_drive_a_search() {
    let app = test::init_service(App::new().route("/search", web::get().to(search))).await;

    let r = send!(&app, get("/search?q=rust&page=2&limit=25", &[]));
    assert_eq!(
        r.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None"
    );
    let r = send!(&app, get("/search?q=hello+world&active=false", &[]));
    assert_eq!(
        r.ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None"
    );
    let r = send!(
        &app,
        get("/search?q=01234&active=true&tag=a&tag=caf%C3%A9", &[])
    );
    assert_eq!(
        r.ok(),
        r#"q=01234 page=None limit=None active=Some(true) tag=Some(["a", "café"])"#
    );

    assert_eq!(
        send!(&app, get("/search?page=1", &[])).issues(),
        issues(&[(".q", "missing_field")])
    );
    assert_eq!(
        send!(&app, get("/search", &[])).issues(),
        issues(&[(".q", "missing_field")])
    );
    assert_eq!(
        send!(&app, get("/search?q=test&limit=500", &[])).issues(),
        issues(&[(".limit", "too_big")])
    );
    assert_eq!(
        send!(&app, get("/search?q=x&page=abc", &[])).issues(),
        issues(&[(".page", "invalid_type")])
    );
}

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Slug {
        pub slug: String => vld::string(),
    }
}

async fn user(p: VldPath<UserPath>) -> HttpResponse {
    HttpResponse::Ok().body(format!("id={}", p.id))
}

async fn slug(p: VldPath<Slug>) -> HttpResponse {
    HttpResponse::Ok().body(p.slug.clone())
}

/// Given `/users/{id}`, `/n/{id:\d+}` (a regex segment) and
/// `/s/{slug}`,
/// when clients request valid, out-of-range, non-numeric and encoded paths,
/// then `VldPath` reads every pattern, decodes what actix leaves encoded
/// (`%2F`, `%25`, `%2B`) and a bad id is a 422 issue at `.id`.
#[actix_rt::test]
async fn path_parameters_address_users() {
    let app = test::init_service(
        App::new()
            .route("/users/{id}", web::get().to(user))
            .route(r"/n/{id:\d+}", web::get().to(user))
            .route("/s/{slug}", web::get().to(slug)),
    )
    .await;

    assert_eq!(send!(&app, get("/users/42", &[])).ok(), "id=42");
    assert_eq!(send!(&app, get("/n/42", &[])).ok(), "id=42");
    assert_eq!(
        send!(&app, get("/n/abc", &[])).status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send!(&app, get("/users/0", &[])).issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send!(&app, get("/users/abc", &[])).issues(),
        issues(&[(".id", "invalid_type")])
    );
    assert_eq!(send!(&app, get("/s/a%2Fb%25c%2Bd", &[])).ok(), "a/b%c+d");
}

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
        pub x_tag: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
        pub dark_mode: Option<bool> => vld::boolean().optional(),
    }
}

async fn protected(h: VldHeaders<AuthHeaders>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "auth={} rid={:?} tag={:?}",
        h.authorization, h.x_request_id, h.x_tag
    ))
}

async fn dashboard(c: VldCookie<Session>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "sid={} theme={:?} dark={:?}",
        c.session_id, c.theme, c.dark_mode
    ))
}

/// Given a dashboard behind `VldHeaders<AuthHeaders>` and
/// `VldCookie<Session>`,
/// when clients send complete, partial, repeated and missing credentials,
/// then header names map to snake_case fields, repeats join with `", "`,
/// non-ASCII values survive, cookies are unquoted, decoded and coerced, and
/// a missing required value is a 422 `missing_field`.
#[actix_rt::test]
async fn headers_and_cookies_open_a_dashboard() {
    let app = test::init_service(
        App::new()
            .route("/protected", web::get().to(protected))
            .route("/dashboard", web::get().to(dashboard)),
    )
    .await;

    let r = send!(
        &app,
        get(
            "/protected",
            &[
                ("Authorization", b"Bearer token123"),
                ("X-Request-Id", b"abc-123")
            ]
        )
    );
    assert_eq!(
        r.ok(),
        r#"auth=Bearer token123 rid=Some("abc-123") tag=None"#
    );
    let r = send!(
        &app,
        get(
            "/protected",
            &[
                ("authorization", b"t"),
                ("x-tag", b"a"),
                ("x-tag", b"caf\xC3\xA9")
            ]
        )
    );
    assert_eq!(r.ok(), r#"auth=t rid=None tag=Some("a, café")"#);
    assert_eq!(
        send!(&app, get("/protected", &[])).issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = send!(
        &app,
        get(
            "/dashboard",
            &[("Cookie", b"session_id=abc123; theme=dark; dark_mode=true")]
        )
    );
    assert_eq!(r.ok(), r#"sid=abc123 theme=Some("dark") dark=Some(true)"#);
    let r = send!(
        &app,
        get("/dashboard", &[("Cookie", b"session_id=\"abc%20def\"")])
    );
    assert_eq!(r.ok(), "sid=abc def theme=None dark=None");
    assert_eq!(
        send!(&app, get("/dashboard", &[])).issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// Given a handler that returns a `VldActixError` built from its own check,
/// when it fails,
/// then the response is the same 422 JSON body the extractors send, and the
/// error exposes status, body and the `VldError`.
#[actix_rt::test]
async fn handlers_return_errors_of_their_own() {
    fn check(name: &str) -> Result<(), VldActixError> {
        NewUser::parse_value(&json!({"name": name, "age": 1}))
            .map(|_| ())
            .map_err(Into::into)
    }
    async fn rename(name: web::Path<String>) -> Result<HttpResponse, VldActixError> {
        check(&name)?;
        Ok(HttpResponse::Ok().body(name.into_inner()))
    }
    let app = test::init_service(App::new().route("/rename/{name}", web::post().to(rename))).await;
    assert_eq!(
        send!(&app, test::TestRequest::post().uri("/rename/Bo")).ok(),
        "Bo"
    );
    let r = send!(&app, test::TestRequest::post().uri("/rename/B"));
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));

    let e = check("B").unwrap_err();
    assert_eq!(e.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.body(), r.json());
    assert_eq!(e.error().issues.len(), 1);
    assert!(e.to_string().starts_with("Validation failed"));
    let too_large = VldActixError::from(vld_web::common::VldHttpError::payload_too_large());
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
