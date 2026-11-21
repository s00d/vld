//! Stories of a salvo API using every `vld-salvo` extractor: one journey
//! per test, each request through a real `Service`.

use salvo::extract::Extractible;
use salvo::extract::metadata::{SourceFrom, SourceParser};
use salvo::http::HeaderValue;
use salvo::http::request::SecureMaxSize;
use salvo::prelude::*;
use salvo::test::{RequestBuilder, ResponseExt, TestClient};
use serde_json::{Value, json};
use vld_salvo::prelude::*;

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
        assert!(
            self.content_type
                .as_deref()
                .unwrap()
                .starts_with("application/json")
        );
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
        assert_eq!(self.json(), json!({"error": "Payload too large"}));
    }
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

async fn send(service: &Service, req: RequestBuilder) -> Reply {
    let mut resp = req.send(service).await;
    let status = resp.status_code.unwrap_or(StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    let text = resp.take_string().await.unwrap();
    Reply {
        status,
        content_type,
        text,
    }
}

fn get(uri: &str, headers: &[(&'static str, &[u8])]) -> RequestBuilder {
    let mut req = TestClient::get(format!("http://localhost{uri}"));
    for (k, v) in headers {
        req = req.add_header(*k, HeaderValue::from_bytes(v).unwrap(), false);
    }
    req
}

fn post(uri: &str, content_type: &str, body: impl Into<Vec<u8>>) -> RequestBuilder {
    TestClient::post(format!("http://localhost{uri}"))
        .add_header("content-type", content_type.to_string(), true)
        .body(body.into())
}

const JSON: &str = "application/json";
const FORM: &str = "application/x-www-form-urlencoded";

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Login {
        pub username: String => vld::string().min(3),
        pub age: i64 => vld::number().int().min(0),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

#[handler]
async fn create(body: VldJson<NewUser>, res: &mut Response) {
    res.render(Json(json!({"name": body.name, "email": body.email})));
}

#[handler]
async fn login(f: VldForm<Login>, res: &mut Response) {
    res.render(format!("{}:{}:{}", f.username, f.age, f.tags.join(",")));
}

fn routes() -> Router {
    Router::new()
        .push(Router::with_path("users").post(create))
        .push(Router::with_path("login").post(login))
}

/// Given a user API with a JSON `POST /users` and a form `POST /login`,
/// when clients send valid bodies, schema errors, malformed JSON, wrong
/// media types and bodies above the default limit,
/// then valid bodies reach the handler and every failure is the documented
/// JSON error: 422 with `(path, code)` issues, 413, 415; Salvo's 64 KiB
/// built-in `secure_max_size` counts as unset (2 MiB applies).
#[tokio::test]
async fn bodies_create_users_and_log_them_in() {
    let service = Service::new(routes());

    let r = send(
        &service,
        post(
            "/users",
            JSON,
            r#"{"name":"Alice","email":"alice@example.com"}"#,
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"{"email":"alice@example.com","name":"Alice"}"#);
    let r = send(
        &service,
        post("/users", JSON, r#"{"name":"A","email":"bad"}"#),
    )
    .await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let r = send(&service, post("/users", JSON, "{}")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".email", "missing_field")])
    );
    let r = send(&service, post("/users", JSON, "not json at all")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send(&service, post("/users", "text/plain", "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(r.json()["error"], "Unsupported Media Type");
    let r = send(&service, post("/login", JSON, "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let r = send(
        &service,
        post("/login", FORM, "username=alice+b&age=30&tags=a&tags=b"),
    )
    .await;
    assert_eq!(r.ok(), "alice b:30:a,b");
    let r = send(&service, post("/login", FORM, "username=al&age=-1&tags=x")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".age", "too_small")])
    );

    // 128 KiB is above Salvo's built-in 64 KiB but within the shared 2 MiB.
    let name = "x".repeat(128 * 1024);
    let r = send(
        &service,
        post(
            "/users",
            JSON,
            format!(r#"{{"name":"{name}","email":"a@b.co"}}"#),
        ),
    )
    .await;
    assert_eq!(r.issues(), issues(&[(".name", "too_big")]));
    let big = "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT);
    send(
        &service,
        post("/users", JSON, format!(r#"{{"name":"{big}"}}"#)),
    )
    .await
    .too_large();
    send(&service, post("/login", FORM, format!("username={big}")))
        .await
        .too_large();
}

/// Given the same routes behind an explicit `SecureMaxSize` of 1 KiB,
/// when bodies are larger,
/// then they are 413 with the JSON body; smaller ones pass.
#[tokio::test]
async fn an_explicit_secure_max_size_is_the_limit() {
    let service = Service::new(Router::new().hoop(SecureMaxSize::new(1024)).push(routes()));
    let big = "x".repeat(2048);
    send(
        &service,
        post("/users", JSON, format!(r#"{{"name":"{big}"}}"#)),
    )
    .await
    .too_large();
    send(&service, post("/login", FORM, format!("username={big}")))
        .await
        .too_large();
    let r = send(
        &service,
        post("/login", FORM, "username=alice&age=1&tags=a"),
    )
    .await;
    assert_eq!(r.ok(), "alice:1:a");
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

#[handler]
async fn search(p: VldQuery<Search>, res: &mut Response) {
    res.render(format!(
        "q={} page={:?} limit={:?} active={:?} tag={:?}",
        p.q, p.page, p.limit, p.active, p.tag
    ));
}

/// Given a `GET /search` handler reading `VldQuery<Search>`,
/// when clients send full, partial, encoded and invalid query strings,
/// then values are decoded and coerced by the schema and a bad value is a
/// 422 issue at its key.
#[tokio::test]
async fn query_strings_drive_a_search() {
    let service = Service::new(Router::with_path("search").get(search));

    let r = send(&service, get("/search?q=rust&page=2&limit=25", &[])).await;
    assert_eq!(
        r.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None"
    );
    let r = send(&service, get("/search?q=hello+world&active=false", &[])).await;
    assert_eq!(
        r.ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None"
    );
    let r = send(
        &service,
        get("/search?q=01234&active=true&tag=a&tag=caf%C3%A9", &[]),
    )
    .await;
    assert_eq!(
        r.ok(),
        r#"q=01234 page=None limit=None active=Some(true) tag=Some(["a", "café"])"#
    );

    let r = send(&service, get("/search?page=0&limit=999", &[])).await;
    assert_eq!(
        r.issues(),
        issues(&[
            (".q", "missing_field"),
            (".page", "too_small"),
            (".limit", "too_big")
        ])
    );
    assert_eq!(
        send(&service, get("/search", &[])).await.issues(),
        issues(&[(".q", "missing_field")])
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

#[handler]
async fn get_user(p: VldPath<UserId>, res: &mut Response) {
    res.render(Json(json!({"id": p.id, "name": "Alice"})));
}

/// Given `/users/{id}`,
/// when clients request valid, out-of-range and non-numeric ids,
/// then `VldPath` reads the route parameter and a bad id is a 422 issue at
/// `.id`.
#[tokio::test]
async fn path_parameters_address_users() {
    let service = Service::new(Router::with_path("users/{id}").get(get_user));

    assert_eq!(
        send(&service, get("/users/42", &[])).await.json(),
        json!({"id": 42, "name": "Alice"})
    );
    assert_eq!(
        send(&service, get("/users/0", &[])).await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(&service, get("/users/abc", &[])).await.issues(),
        issues(&[(".id", "invalid_type")])
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

#[handler]
async fn protected(h: VldHeaders<AuthHeaders>, res: &mut Response) {
    res.render(format!(
        "auth={} rid={:?} tag={:?}",
        h.authorization, h.x_request_id, h.x_tag
    ));
}

#[handler]
async fn dashboard(c: VldCookie<Session>, res: &mut Response) {
    res.render(format!(
        "sid={} theme={:?} dark={:?}",
        c.session_id, c.theme, c.dark_mode
    ));
}

/// Given a dashboard behind `VldHeaders<AuthHeaders>` and
/// `VldCookie<Session>`,
/// when clients send complete, partial, repeated and missing credentials,
/// then header names map to snake_case fields, repeats join with `", "`,
/// cookies are unquoted, decoded and coerced, and a missing required value
/// is a 422 `missing_field`.
#[tokio::test]
async fn headers_and_cookies_open_a_dashboard() {
    let service = Service::new(
        Router::new()
            .push(Router::with_path("protected").get(protected))
            .push(Router::with_path("dashboard").get(dashboard)),
    );

    let r = send(
        &service,
        get(
            "/protected",
            &[
                ("authorization", b"Bearer token123"),
                ("x-request-id", b"abc-123"),
            ],
        ),
    )
    .await;
    assert_eq!(
        r.ok(),
        r#"auth=Bearer token123 rid=Some("abc-123") tag=None"#
    );
    let r = send(
        &service,
        get(
            "/protected",
            &[
                ("authorization", b"t"),
                ("x-tag", b"a"),
                ("x-tag", b"caf\xC3\xA9"),
            ],
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"auth=t rid=None tag=Some("a, café")"#);
    assert_eq!(
        send(&service, get("/protected", &[])).await.issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = send(
        &service,
        get(
            "/dashboard",
            &[("cookie", b"session_id=abc123; theme=dark; dark_mode=true")],
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"sid=abc123 theme=Some("dark") dark=Some(true)"#);
    let r = send(
        &service,
        get("/dashboard", &[("cookie", b"session_id=\"abc%20def\"")]),
    )
    .await;
    assert_eq!(r.ok(), "sid=abc def theme=None dark=None");
    assert_eq!(
        send(&service, get("/dashboard", &[])).await.issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// Given a handler returning `Result<_, VldSalvoError>` from its own check,
/// and salvo reading the extractors' metadata (e.g. for OpenAPI),
/// when the check fails,
/// then the response is the same 422 JSON body the extractors send, the
/// error exposes status, body and the `VldError`, and each extractor
/// declares its request source.
#[tokio::test]
async fn handlers_return_errors_of_their_own() {
    fn check(name: &str) -> Result<(), VldSalvoError> {
        NewUser::parse_value(&json!({"name": name, "email": "a@b.co"}))
            .map(|_| ())
            .map_err(Into::into)
    }
    #[handler]
    async fn rename(req: &mut Request) -> Result<String, VldSalvoError> {
        let name: String = req.param("name").unwrap();
        check(&name)?;
        Ok(name)
    }
    let service = Service::new(Router::with_path("rename/{name}").post(rename));
    assert_eq!(
        send(&service, post("/rename/Bo", "text/plain", ""))
            .await
            .ok(),
        "Bo"
    );
    let r = send(&service, post("/rename/B", "text/plain", "")).await;
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));

    let e = check("B").unwrap_err();
    assert_eq!(e.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.body(), r.json());
    assert_eq!(e.error().issues.len(), 1);
    assert!(e.to_string().starts_with("Validation failed"));
    assert_eq!(VldSalvoError::new(e.error().clone()).body(), e.body());
    let too_large = VldSalvoError::from(vld_http_common::VldHttpError::payload_too_large());
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let sources = [
        (
            <VldJson<NewUser> as Extractible>::metadata(),
            "VldJson",
            SourceFrom::Body,
            SourceParser::Json,
        ),
        (
            <VldForm<NewUser> as Extractible>::metadata(),
            "VldForm",
            SourceFrom::Body,
            SourceParser::MultiMap,
        ),
        (
            <VldQuery<NewUser> as Extractible>::metadata(),
            "VldQuery",
            SourceFrom::Query,
            SourceParser::MultiMap,
        ),
        (
            <VldPath<NewUser> as Extractible>::metadata(),
            "VldPath",
            SourceFrom::Param,
            SourceParser::MultiMap,
        ),
        (
            <VldHeaders<NewUser> as Extractible>::metadata(),
            "VldHeaders",
            SourceFrom::Header,
            SourceParser::MultiMap,
        ),
        (
            <VldCookie<NewUser> as Extractible>::metadata(),
            "VldCookie",
            SourceFrom::Cookie,
            SourceParser::MultiMap,
        ),
    ];
    for (meta, name, from, parser) in sources {
        assert_eq!(meta.name, name);
        assert_eq!(meta.default_sources.len(), 1, "{name}");
        assert_eq!(
            (meta.default_sources[0].from, meta.default_sources[0].parser),
            (from, parser),
            "{name}"
        );
    }
}
