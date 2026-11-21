//! Stories of a poem API using every `vld-poem` extractor: one journey per
//! test, each request through a real `Route`.

use poem::http::{HeaderValue, Method, StatusCode};
use poem::{Endpoint, EndpointExt, Route, get, handler, post};
use serde_json::{Value, json};
use vld_poem::prelude::*;

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
        assert_eq!(self.text, r#"{"error":"Payload too large"}"#);
    }
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

async fn send(app: &impl Endpoint, req: poem::Request) -> Reply {
    let resp = app.get_response(req).await;
    let status = resp.status();
    let content_type = resp.content_type().map(str::to_string);
    let text = resp.into_body().into_string().await.unwrap();
    Reply {
        status,
        content_type,
        text,
    }
}

fn get_(uri: &str, headers: &[(&str, &[u8])]) -> poem::Request {
    let mut req = poem::Request::builder().uri(uri.parse().unwrap());
    for (k, v) in headers {
        req = req.header(*k, HeaderValue::from_bytes(v).unwrap());
    }
    req.finish()
}

fn post_(uri: &str, content_type: &str, body: impl Into<poem::Body>) -> poem::Request {
    poem::Request::builder()
        .method(Method::POST)
        .uri(uri.parse().unwrap())
        .content_type(content_type)
        .body(body)
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
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

#[handler]
async fn create(user: VldJson<NewUser>) -> poem::web::Json<Value> {
    poem::web::Json(json!({"name": user.name, "email": user.email}))
}

#[handler]
async fn login(VldForm(f): VldForm<Login>) -> String {
    format!("user={}", f.username)
}

/// Reads the body twice: the second extractor finds it already taken.
#[handler]
async fn twice(_a: VldJson<NewUser>, _b: VldJson<NewUser>) -> String {
    "unreachable".into()
}

/// Given a user API with a JSON `POST /users`, a form `POST /login` and a
/// handler that reads the body twice,
/// when clients send valid bodies, schema errors, wrong JSON, wrong media
/// types, non-UTF-8 forms and bodies above the default 2 MiB limit,
/// then valid bodies reach the handler and every failure is the documented
/// JSON error: 422 with `(path, code)` issues, 413, 415.
#[tokio::test]
async fn bodies_create_users_and_log_them_in() {
    let app = Route::new()
        .at("/users", post(create))
        .at("/login", post(login))
        .at("/twice", post(twice));

    let r = send(
        &app,
        post_(
            "/users",
            JSON,
            r#"{"name":"Alice","email":"alice@example.com"}"#,
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"{"email":"alice@example.com","name":"Alice"}"#);
    let r = send(&app, post_("/users", JSON, r#"{"name":"A","email":"bad"}"#)).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let r = send(&app, post_("/users", JSON, "{}")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".email", "missing_field")])
    );
    let r = send(&app, post_("/users", JSON, "[1]")).await;
    assert_eq!(r.issues(), issues(&[("", "invalid_type")]));
    let r = send(&app, post_("/users", JSON, "not json")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send(&app, post_("/users", "text/plain", "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(r.json()["error"], "Unsupported Media Type");
    let r = send(&app, post_("/login", JSON, "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // The second body extractor finds the body taken: a 422 parse error.
    let r = send(
        &app,
        post_("/twice", JSON, r#"{"name":"Al","email":"a@b.co"}"#),
    )
    .await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));

    // Form body.
    assert_eq!(
        send(
            &app,
            post_("/login", FORM, "username=alice&password=secret1234")
        )
        .await
        .ok(),
        "user=alice"
    );
    let r = send(&app, post_("/login", FORM, "username=al&password=123")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".password", "too_small")])
    );
    let r = send(&app, post_("/login", FORM, b"username=\xFF\xFE".to_vec())).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));

    // Above the default limit.
    let big = "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT);
    send(
        &app,
        post_("/users", JSON, format!(r#"{{"name":"{big}"}}"#)),
    )
    .await
    .too_large();
    send(&app, post_("/login", FORM, format!("username={big}")))
        .await
        .too_large();
}

/// Given routes carrying `VldBodyLimit` request data,
/// when the limit is 16 bytes or 4 MiB,
/// then a body above it is 413, a body of exactly the limit passes, and a
/// 3 MiB body passes the raised limit.
#[tokio::test]
async fn the_body_limit_is_request_data() {
    let app = Route::new()
        .at("/small", post(login).data(VldBodyLimit(16)))
        .at("/large", post(create).data(VldBodyLimit(4 << 20)));
    send(
        &app,
        post_("/small", FORM, "username=alice&password=secret1234"),
    )
    .await
    .too_large();
    let r = send(&app, post_("/small", FORM, "username=al&pass")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".password", "missing_field")])
    );

    let name = "x".repeat(3 << 20);
    let r = send(
        &app,
        post_(
            "/large",
            JSON,
            format!(r#"{{"name":"{name}","email":"a@b.co"}}"#),
        ),
    )
    .await;
    assert_eq!(r.issues(), issues(&[(".name", "too_big")]));
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
async fn search(VldQuery(p): VldQuery<Search>) -> String {
    format!(
        "q={} page={:?} limit={:?} active={:?} tag={:?}",
        p.q, p.page, p.limit, p.active, p.tag
    )
}

/// Given a `GET /search` handler reading `VldQuery<Search>`,
/// when clients send full, partial, encoded and invalid query strings,
/// then values are decoded and coerced by the schema and a bad value is a
/// 422 issue at its key.
#[tokio::test]
async fn query_strings_drive_a_search() {
    let app = Route::new().at("/search", get(search));

    let r = send(&app, get_("/search?q=rust&page=2&limit=25", &[])).await;
    assert_eq!(
        r.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None"
    );
    let r = send(&app, get_("/search?q=hello+world&active=false", &[])).await;
    assert_eq!(
        r.ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None"
    );
    let r = send(
        &app,
        get_("/search?q=01234&active=true&tag=a&tag=caf%C3%A9", &[]),
    )
    .await;
    assert_eq!(
        r.ok(),
        r#"q=01234 page=None limit=None active=Some(true) tag=Some(["a", "café"])"#
    );

    assert_eq!(
        send(&app, get_("/search?page=0&limit=200", &[]))
            .await
            .issues(),
        issues(&[
            (".q", "missing_field"),
            (".page", "too_small"),
            (".limit", "too_big")
        ])
    );
    assert_eq!(
        send(&app, get_("/search", &[])).await.issues(),
        issues(&[(".q", "missing_field")])
    );
    assert_eq!(
        send(&app, get_("/search?q=x&page=abc", &[])).await.issues(),
        issues(&[(".page", "invalid_type")])
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

#[handler]
async fn get_user(p: VldPath<UserPath>) -> String {
    format!("id={}", p.id)
}

/// Given `/users/:id`,
/// when clients request valid, out-of-range and non-numeric ids, also with
/// an `id` in the query string,
/// then `VldPath` reads the route parameter (never the query) and a bad id
/// is a 422 issue at `.id`.
#[tokio::test]
async fn path_parameters_address_users() {
    let app = Route::new().at("/users/:id", get(get_user));

    assert_eq!(send(&app, get_("/users/42", &[])).await.ok(), "id=42");
    assert_eq!(send(&app, get_("/users/42?id=7", &[])).await.ok(), "id=42");
    assert_eq!(
        send(&app, get_("/users/0", &[])).await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(&app, get_("/users/abc", &[])).await.issues(),
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
async fn protected(VldHeaders(h): VldHeaders<AuthHeaders>) -> String {
    format!(
        "auth={} rid={:?} tag={:?}",
        h.authorization, h.x_request_id, h.x_tag
    )
}

#[handler]
async fn dashboard(VldCookie(c): VldCookie<Session>) -> String {
    format!(
        "sid={} theme={:?} dark={:?}",
        c.session_id, c.theme, c.dark_mode
    )
}

/// Given a dashboard behind `VldHeaders<AuthHeaders>` and
/// `VldCookie<Session>`,
/// when clients send complete, partial, repeated and missing credentials,
/// then header names map to snake_case fields, repeats join with `", "`,
/// cookies are unquoted, decoded and coerced, and a missing required value
/// is a 422 `missing_field`.
#[tokio::test]
async fn headers_and_cookies_open_a_dashboard() {
    let app = Route::new()
        .at("/protected", get(protected))
        .at("/dashboard", get(dashboard));

    let r = send(
        &app,
        get_(
            "/protected",
            &[
                ("Authorization", b"Bearer token123"),
                ("X-Request-Id", b"abc-123"),
            ],
        ),
    )
    .await;
    assert_eq!(
        r.ok(),
        r#"auth=Bearer token123 rid=Some("abc-123") tag=None"#
    );
    let r = send(
        &app,
        get_(
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
        send(&app, get_("/protected", &[])).await.issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = send(
        &app,
        get_(
            "/dashboard",
            &[("Cookie", b"session_id=abc123; theme=dark; dark_mode=true")],
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"sid=abc123 theme=Some("dark") dark=Some(true)"#);
    let r = send(
        &app,
        get_("/dashboard", &[("Cookie", b"session_id=\"abc%20def\"")]),
    )
    .await;
    assert_eq!(r.ok(), "sid=abc def theme=None dark=None");
    assert_eq!(
        send(&app, get_("/dashboard", &[])).await.issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// Given a handler that returns a `VldPoemError` built from its own check,
/// when it fails,
/// then the response is the same 422 JSON body the extractors send, and the
/// error exposes status, body and the `VldError`.
#[tokio::test]
async fn handlers_return_errors_of_their_own() {
    fn check(name: &str) -> Result<(), VldPoemError> {
        NewUser::parse_value(&json!({"name": name, "email": "a@b.co"}))
            .map(|_| ())
            .map_err(Into::into)
    }
    #[handler]
    async fn rename(poem::web::Path(name): poem::web::Path<String>) -> poem::Result<String> {
        check(&name)?;
        Ok(name)
    }
    let app = Route::new().at("/rename/:name", post(rename));
    assert_eq!(
        send(&app, post_("/rename/Bo", "text/plain", "")).await.ok(),
        "Bo"
    );
    let r = send(&app, post_("/rename/B", "text/plain", "")).await;
    assert_eq!(r.issues(), issues(&[(".name", "too_small")]));

    // Middleware sees the error's status before it is rendered.
    let seen = app.around(|ep, req| async move {
        let status = ep.call(req).await.err().map(|e| e.status());
        Ok(format!("{status:?}"))
    });
    let observed = send(&seen, post_("/rename/B", "text/plain", "")).await;
    assert_eq!(observed.ok(), "Some(422)");

    let e = check("B").unwrap_err();
    assert_eq!(e.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.body(), r.json());
    assert_eq!(e.error().issues.len(), 1);
    assert!(e.to_string().starts_with("Validation failed"));
    let too_large = VldPoemError::from(vld_http_common::VldHttpError::payload_too_large());
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
