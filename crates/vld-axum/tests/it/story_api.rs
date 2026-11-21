//! Stories of an axum API using every `vld-axum` extractor: one journey per
//! test, each request through the real `Router`.

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use vld_axum::prelude::*;

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
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

async fn send(app: &Router, req: Request<Body>) -> Reply {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(http::header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    Reply {
        status,
        content_type,
        text: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

async fn get_(app: &Router, uri: &str, headers: &[(&str, &[u8])]) -> Reply {
    let mut req = Request::builder().uri(uri);
    for (k, v) in headers {
        req = req.header(*k, http::HeaderValue::from_bytes(v).unwrap());
    }
    send(app, req.body(Body::empty()).unwrap()).await
}

async fn post_(app: &Router, uri: &str, content_type: &str, body: impl Into<Body>) -> Reply {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", content_type)
        .body(body.into())
        .unwrap();
    send(app, req).await
}

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

/// Given a user API with a JSON `POST /users` and a form `POST /login`
/// behind a 64-byte `DefaultBodyLimit`,
/// when clients send valid bodies, schema errors, wrong JSON and oversized
/// bodies,
/// then valid bodies reach the handler (fields through `Deref`) and every
/// failure is the documented JSON error: 422 with `(path, code)` issues,
/// 413, 415.
#[tokio::test]
async fn bodies_create_users_and_log_them_in() {
    async fn create(user: VldJson<NewUser>) -> String {
        format!("{}:{}", user.name, user.age)
    }
    async fn login(VldForm(f): VldForm<Login>) -> String {
        format!("user={}", f.username)
    }
    let app = Router::new()
        .route("/users", post(create))
        .route("/login", post(login))
        .layer(DefaultBodyLimit::max(64));
    let json = "application/json";
    let form = "application/x-www-form-urlencoded";

    // JSON body.
    assert_eq!(
        post_(&app, "/users", json, r#"{"name":"Alice","age":25}"#)
            .await
            .ok(),
        "Alice:25"
    );
    let r = post_(&app, "/users", json, r#"{"name":"A","age":-1}"#).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".age", "too_small")])
    );
    let r = post_(&app, "/users", json, "{}").await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".age", "missing_field")])
    );
    let r = post_(&app, "/users", json, "[1,2,3]").await;
    assert_eq!(r.issues(), issues(&[("", "invalid_type")]));
    let r = post_(&app, "/users", json, "not json").await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = post_(&app, "/users", "text/plain", "{}").await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(r.json()["error"], "Unsupported Media Type");

    // Above the configured limit: 413 with the exact body, JSON and form.
    let long = format!(r#"{{"name":"{}","age":1}}"#, "x".repeat(64));
    let r = post_(&app, "/users", json, long).await;
    assert_eq!(
        (r.status, r.text.as_str()),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            r#"{"error":"Payload too large"}"#
        )
    );
    assert_eq!(r.content_type.as_deref(), Some("application/json"));
    let r = post_(&app, "/login", form, format!("username={}", "x".repeat(64))).await;
    assert_eq!(
        (r.status, r.text.as_str()),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            r#"{"error":"Payload too large"}"#
        )
    );

    // Form body.
    assert_eq!(
        post_(&app, "/login", form, "username=alice&password=secret1234")
            .await
            .ok(),
        "user=alice"
    );
    let r = post_(&app, "/login", form, "username=al&password=123").await;
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".password", "too_small")])
    );
    let r = post_(&app, "/login", form, "").await;
    assert_eq!(
        r.issues(),
        issues(&[
            (".username", "missing_field"),
            (".password", "missing_field")
        ])
    );
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
        pub active: Option<bool> => vld::boolean().optional(),
        pub tag: Option<Vec<String>> => vld::array(vld::string()).optional(),
        pub phone: Option<String> => vld::string().optional(),
    }
}

/// Given a `GET /search` handler reading `VldQuery<Search>`,
/// when clients send full, partial, encoded and invalid query strings,
/// then values are decoded and coerced by the schema (numbers, booleans,
/// repeated keys, numeric-looking strings kept as text) and a bad value is a
/// 422 issue at its key.
#[tokio::test]
async fn query_strings_drive_a_search() {
    async fn search(VldQuery(p): VldQuery<Search>) -> String {
        format!(
            "q={} page={:?} limit={:?} active={:?} tag={:?} phone={:?}",
            p.q, p.page, p.limit, p.active, p.tag, p.phone
        )
    }
    let app = Router::new().route("/search", get(search));

    assert_eq!(
        get_(&app, "/search?q=rust&page=2&limit=25", &[]).await.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None phone=None"
    );
    assert_eq!(
        get_(&app, "/search?q=hello+world&active=false", &[])
            .await
            .ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None phone=None"
    );
    assert_eq!(
        get_(
            &app,
            "/search?q=caf%C3%A9&active=true&tag=a&tag=caf%C3%A9&phone=%2B15551234567",
            &[]
        )
        .await
        .ok(),
        r#"q=café page=None limit=None active=Some(true) tag=Some(["a", "café"]) phone=Some("+15551234567")"#
    );
    assert_eq!(
        get_(&app, "/search?q=1&phone=", &[]).await.ok(),
        r#"q=1 page=None limit=None active=None tag=None phone=Some("")"#
    );

    assert_eq!(
        get_(&app, "/search?page=1", &[]).await.issues(),
        issues(&[(".q", "missing_field")])
    );
    assert_eq!(
        get_(&app, "/search", &[]).await.issues(),
        issues(&[(".q", "missing_field")])
    );
    assert_eq!(
        get_(&app, "/search?q=test&limit=500", &[]).await.issues(),
        issues(&[(".limit", "too_big")])
    );
    assert_eq!(
        get_(&app, "/search?q=x&active=yes", &[]).await.issues(),
        issues(&[(".active", "invalid_type")])
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
    pub struct FilePath {
        pub id: String => vld::string(),
        pub rest: String => vld::string(),
    }
}

/// Given `/users/{id}` at the root and `/files/{id}/{*rest}` in a router
/// nested under `/api`,
/// when clients request valid, out-of-range, non-numeric and invalid-UTF-8
/// paths,
/// then `VldPath` reads the template parameters (also after nesting and
/// through a wildcard), decoding invalid UTF-8 lossily, and a bad id is a
/// 422 issue at `.id`.
#[tokio::test]
async fn path_parameters_address_users_and_files() {
    async fn user(VldPath(p): VldPath<UserPath>) -> String {
        format!("id={}", p.id)
    }
    async fn file(p: VldPath<FilePath>) -> String {
        format!("{}|{}", p.id, p.rest)
    }
    let app = Router::new().route("/users/{id}", get(user)).nest(
        "/api",
        Router::new().route("/files/{id}/{*rest}", get(file)),
    );

    assert_eq!(get_(&app, "/users/42", &[]).await.ok(), "id=42");
    assert_eq!(
        get_(&app, "/users/0", &[]).await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        get_(&app, "/users/abc", &[]).await.issues(),
        issues(&[(".id", "invalid_type")])
    );

    assert_eq!(get_(&app, "/api/files/7/a/b", &[]).await.ok(), "7|a/b");
    assert_eq!(
        get_(&app, "/api/files/a%FF/x%2Fy/z", &[]).await.ok(),
        "a\u{FFFD}|x/y/z"
    );
    assert_eq!(get_(&app, "/api/files/%FF/r", &[]).await.ok(), "\u{FFFD}|r");
    assert_eq!(
        get_(&app, "/api/files/k/%FFz", &[]).await.ok(),
        "k|\u{FFFD}z"
    );

    // The same lossy decoding on an un-nested route.
    let root = Router::new().route("/u/{id}/{*rest}", get(file));
    assert_eq!(
        get_(&root, "/u/%C3%A9%FF/%41", &[]).await.ok(),
        "é\u{FFFD}|A"
    );
}

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
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

/// Given a dashboard behind `VldHeaders<AuthHeaders>` and
/// `VldCookie<Session>`,
/// when clients send complete, partial and missing credentials,
/// then header names map to snake_case fields, non-ASCII values survive,
/// cookies are unquoted and coerced, optional values may be absent and a
/// missing required one is a 422 `missing_field`.
#[tokio::test]
async fn headers_and_cookies_open_a_dashboard() {
    async fn protected(VldHeaders(h): VldHeaders<AuthHeaders>) -> String {
        format!("auth={} rid={:?}", h.authorization, h.x_request_id)
    }
    async fn dashboard(VldCookie(c): VldCookie<Session>) -> String {
        format!(
            "sid={} theme={:?} dark={:?}",
            c.session_id, c.theme, c.dark_mode
        )
    }
    let app = Router::new()
        .route("/protected", get(protected))
        .route("/dashboard", get(dashboard));

    let r = get_(
        &app,
        "/protected",
        &[
            ("Authorization", b"Bearer token123"),
            ("X-Request-Id", b"abc-123"),
        ],
    )
    .await;
    assert_eq!(r.ok(), r#"auth=Bearer token123 rid=Some("abc-123")"#);
    let r = get_(&app, "/protected", &[("authorization", b"caf\xC3\xA9")]).await;
    assert_eq!(r.ok(), "auth=café rid=None");
    assert_eq!(
        get_(&app, "/protected", &[]).await.issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = get_(
        &app,
        "/dashboard",
        &[("Cookie", b"session_id=abc123; theme=dark; dark_mode=true")],
    )
    .await;
    assert_eq!(r.ok(), r#"sid=abc123 theme=Some("dark") dark=Some(true)"#);
    let r = get_(&app, "/dashboard", &[("Cookie", b"session_id=\"abc\"")]).await;
    assert_eq!(r.ok(), "sid=abc theme=None dark=None");
    assert_eq!(
        get_(&app, "/dashboard", &[]).await.issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// Given a handler that builds a `VldJsonRejection` itself (from a
/// `VldError` of its own check),
/// when it returns the rejection,
/// then the response is the same 422 JSON body the extractors send, and the
/// rejection exposes status, body and error.
#[tokio::test]
async fn handlers_return_rejections_of_their_own() {
    fn check(name: &str) -> Result<(), VldJsonRejection> {
        let err = NewUser::parse_value(&json!({"name": name, "age": 1})).err();
        err.map_or(Ok(()), |e| Err(e.into()))
    }
    async fn rename(
        axum::extract::Path(name): axum::extract::Path<String>,
    ) -> Result<String, VldJsonRejection> {
        check(&name).map(|()| name)
    }
    let app = Router::new().route("/rename/{name}", post(rename));
    let r = send(
        &app,
        Request::post("/rename/Bo").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(r.ok(), "Bo");
    let r = send(
        &app,
        Request::post("/rename/B").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));

    let rejection = check("B").unwrap_err();
    assert_eq!(rejection.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(rejection.body(), r.json());
    assert_eq!(rejection.error().issues.len(), 1);
    assert!(rejection.to_string().starts_with("Validation failed"));
    let debug = format!("{rejection:?}");
    assert!(
        debug.starts_with("VldJsonRejection") && debug.contains("422"),
        "{debug}"
    );
    let as_error: &dyn std::error::Error = &rejection;
    assert_eq!(as_error.to_string(), rejection.to_string());
    let too_large = VldJsonRejection::from(vld_http_common::VldHttpError::payload_too_large());
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
