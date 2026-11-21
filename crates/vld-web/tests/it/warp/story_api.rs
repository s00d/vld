//! Stories of a warp API built from every `vld-warp` filter and recovered
//! with `handle_rejection`: one journey per test, each request through the
//! real filter chain.

use http_body_util::{BodyExt, Full};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use vld_web::warp::prelude::*;
use warp::Filter;
use warp::http::{HeaderValue, StatusCode};

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

    /// `(status, {"error": ...})` of a generic error body.
    #[track_caller]
    fn error(&self) -> (u16, String) {
        let body = self.json();
        assert_eq!(body.as_object().unwrap().len(), 1, "{body}");
        (
            self.status.as_u16(),
            body["error"].as_str().unwrap().to_string(),
        )
    }
}

fn issues(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// Send through `warp::service` (unlike `warp::test::request()` it keeps
/// repeated headers).
async fn send<F>(
    filter: &F,
    method: &str,
    uri: &str,
    headers: &[(&str, &[u8])],
    body: impl Into<bytes::Bytes>,
) -> Reply
where
    F: Filter + Clone + Send + Sync + 'static,
    F::Extract: warp::Reply,
    F::Error: Into<warp::Rejection>,
{
    let mut req = warp::http::Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        req = req.header(*k, HeaderValue::from_bytes(v).unwrap());
    }
    let req = req.body(Full::new(body.into())).unwrap();
    let resp = warp::service(filter.clone()).oneshot(req).await.unwrap();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        content_type,
        text: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

const JSON: &[u8] = b"application/json";
const FORM: &[u8] = b"application/x-www-form-urlencoded";

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

fn body_routes()
-> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone {
    let create = warp::post()
        .and(warp::path!("users"))
        .and(vld_json::<NewUser>())
        .map(|u: NewUser| warp::reply::json(&json!({"name": u.name, "email": u.email})));
    let login = warp::post()
        .and(warp::path!("login"))
        .and(vld_form::<Named>())
        .map(|n: Named| n.name);
    let json16 = warp::post()
        .and(warp::path!("json16"))
        .and(vld_json_limit::<Named>(16))
        .map(|n: Named| n.name);
    let form16 = warp::post()
        .and(warp::path!("form16"))
        .and(vld_form_limit::<Named>(16))
        .map(|n: Named| n.name);
    create
        .or(login)
        .or(json16)
        .or(form16)
        .recover(handle_rejection)
}

/// Given a user API with `vld_json`, `vld_form` and their `_limit`
/// variants (16 bytes),
/// when clients send valid bodies, schema errors, malformed or empty JSON,
/// wrong (also non-ASCII) media types, non-UTF-8 forms and oversized bodies,
/// then valid bodies reach the handler and every failure is the documented
/// JSON error: 422 with `(path, code)` issues, 413, 415; a body of exactly
/// the limit passes.
#[tokio::test]
async fn bodies_create_users_and_log_them_in() {
    let r = body_routes();
    let ct = |v: &'static [u8]| [("content-type", v)];

    let resp = send(
        &r,
        "POST",
        "/users",
        &ct(JSON),
        r#"{"name":"Alice","email":"alice@example.com"}"#,
    )
    .await;
    assert_eq!(
        resp.json(),
        json!({"name": "Alice", "email": "alice@example.com"})
    );
    let resp = send(
        &r,
        "POST",
        "/users",
        &ct(JSON),
        r#"{"name":"A","email":"bad"}"#,
    )
    .await;
    assert_eq!(
        resp.issues(),
        issues(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let resp = send(&r, "POST", "/users", &ct(JSON), "{}").await;
    assert_eq!(
        resp.issues(),
        issues(&[(".name", "missing_field"), (".email", "missing_field")])
    );
    let resp = send(&r, "POST", "/users", &ct(JSON), "not json at all").await;
    assert_eq!(resp.issues(), issues(&[("", "parse_error")]));
    // No body and no Content-Length: read as empty, invalid JSON (not 411).
    let resp = warp::test::request()
        .method("POST")
        .path("/users")
        .header("content-type", "application/json")
        .reply(&r)
        .await;
    assert_eq!(resp.status(), 422);
    let resp = send(&r, "POST", "/users", &ct(b"text/plain"), "{}").await;
    assert_eq!(resp.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        resp.json()["message"],
        vld_web::common::UNSUPPORTED_MEDIA_TYPE_MESSAGE
    );
    let resp = send(
        &r,
        "POST",
        "/users",
        &ct("application/jsön".as_bytes()),
        "{}",
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let resp = send(&r, "POST", "/users", &[], "{}").await;
    assert_eq!(resp.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let resp = send(&r, "POST", "/login", &ct(JSON), "name=Al").await;
    assert_eq!(
        resp.json()["message"],
        vld_web::common::UNSUPPORTED_FORM_MEDIA_TYPE_MESSAGE
    );

    assert_eq!(
        send(&r, "POST", "/login", &ct(FORM), "name=Al+B")
            .await
            .ok(),
        "Al B"
    );
    let resp = send(&r, "POST", "/login", &ct(FORM), "name=A").await;
    assert_eq!(resp.issues(), issues(&[(".name", "too_small")]));
    let resp = send(&r, "POST", "/login", &ct(FORM), &b"name=\xFF\xFE"[..]).await;
    assert_eq!(resp.issues(), issues(&[("", "parse_error")]));

    // Limits.
    let too_large = (413, "Payload too large".to_string());
    assert_eq!(
        send(
            &r,
            "POST",
            "/json16",
            &ct(JSON),
            r#"{"name":"a-very-long-name"}"#
        )
        .await
        .error(),
        too_large
    );
    assert_eq!(
        send(&r, "POST", "/form16", &ct(FORM), "name=a-very-long-name")
            .await
            .error(),
        too_large
    );
    assert_eq!(
        send(&r, "POST", "/json16", &ct(JSON), r#"{"name":"12345"}"#)
            .await
            .ok(),
        "12345"
    );
    assert_eq!(
        send(&r, "POST", "/form16", &ct(FORM), "name=12345678901")
            .await
            .ok(),
        "12345678901"
    );
    let big = "x".repeat(vld_web::common::DEFAULT_BODY_LIMIT);
    assert_eq!(
        send(
            &r,
            "POST",
            "/users",
            &ct(JSON),
            format!(r#"{{"name":"{big}"}}"#)
        )
        .await
        .error(),
        too_large
    );
    assert_eq!(
        send(&r, "POST", "/login", &ct(FORM), format!("name={big}"))
            .await
            .error(),
        too_large
    );
}

/// Given the same API served over a real socket,
/// when a client sends a chunked JSON body without `Content-Length`,
/// then the body is streamed, validated and answered with 200.
#[tokio::test]
async fn chunked_bodies_are_streamed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(warp::serve(body_routes()).incoming(listener).run());

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let body = r#"{"name":"Al"}"#;
    let req = format!(
        "POST /json16 HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n\
         Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(resp.ends_with("Al"), "{resp}");
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

/// Given a `GET /search` filter with `vld_query::<Search>()`,
/// when clients send full, partial, encoded, empty and invalid query
/// strings,
/// then values are decoded and coerced by the schema and a bad value is a
/// 422 issue at its key.
#[tokio::test]
async fn query_strings_drive_a_search() {
    let r = warp::get()
        .and(warp::path!("search"))
        .and(vld_query::<Search>())
        .map(|p: Search| {
            format!(
                "q={} page={:?} limit={:?} active={:?} tag={:?}",
                p.q, p.page, p.limit, p.active, p.tag
            )
        })
        .recover(handle_rejection);

    let resp = send(&r, "GET", "/search?q=rust&page=2&limit=25", &[], "").await;
    assert_eq!(
        resp.ok(),
        "q=rust page=Some(2) limit=Some(25) active=None tag=None"
    );
    let resp = send(&r, "GET", "/search?q=hello+world&active=false", &[], "").await;
    assert_eq!(
        resp.ok(),
        "q=hello world page=None limit=None active=Some(false) tag=None"
    );
    let resp = send(
        &r,
        "GET",
        "/search?q=01234&active=true&tag=a&tag=caf%C3%A9",
        &[],
        "",
    )
    .await;
    assert_eq!(
        resp.ok(),
        r#"q=01234 page=None limit=None active=Some(true) tag=Some(["a", "café"])"#
    );

    let resp = send(&r, "GET", "/search?page=0&limit=999", &[], "").await;
    assert_eq!(
        resp.issues(),
        issues(&[
            (".q", "missing_field"),
            (".page", "too_small"),
            (".limit", "too_big")
        ])
    );
    assert_eq!(
        send(&r, "GET", "/search", &[], "").await.issues(),
        issues(&[(".q", "missing_field")])
    );
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Slug {
        pub slug: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PostPath {
        pub user_id: i64 => vld::number().int().min(1),
        pub post_id: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CommentPath {
        pub user_id: i64 => vld::number().int().min(1),
        pub post_id: i64 => vld::number().int().min(1),
        pub comment: String => vld::string(),
    }
}

/// Given routes with `vld_param` (one segment), `vld_path` (the tail, by
/// name) and `validate_path_params` (segments taken by warp),
/// when clients request valid, encoded, invalid and too-short paths,
/// then segments are percent-decoded once (`+` kept) and validated, a bad
/// segment is a 422 issue at its name, and a tail with the wrong number of
/// segments does not match (404).
#[tokio::test]
async fn path_segments_address_users_posts_and_comments() {
    let user = warp::path("users")
        .and(vld_param::<UserId>("id"))
        .and(warp::path::end())
        .map(|p: UserId| warp::reply::json(&json!({"id": p.id, "name": "Alice"})));
    let slug = warp::path("s")
        .and(vld_param::<Slug>("slug"))
        .and(warp::path::end())
        .map(|s: Slug| s.slug);
    let post = warp::path("posts")
        .and(vld_path::<PostPath>(&["user_id", "post_id"]))
        .map(|p: PostPath| format!("{}|{}", p.user_id, p.post_id));
    let comment = warp::path("c")
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and_then(|u: String, p: String, c: String| async move {
            validate_path_params::<CommentPath>(&[
                ("user_id", &u),
                ("post_id", &p),
                ("comment", &c),
            ])
        })
        .map(|p: CommentPath| format!("{}/{}/{}", p.user_id, p.post_id, p.comment));
    let r = warp::get()
        .and(user.or(slug).or(post).or(comment))
        .recover(handle_rejection);

    assert_eq!(
        send(&r, "GET", "/users/42", &[], "").await.json(),
        json!({"id": 42, "name": "Alice"})
    );
    assert_eq!(
        send(&r, "GET", "/users/0", &[], "").await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(&r, "GET", "/users/-5", &[], "").await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(&r, "GET", "/users/abc", &[], "").await.issues(),
        issues(&[(".id", "invalid_type")])
    );
    assert_eq!(
        send(&r, "GET", "/s/caf%C3%A9%20au+lait", &[], "")
            .await
            .ok(),
        "café au+lait"
    );

    assert_eq!(
        send(&r, "GET", "/posts/7/a%2Fb", &[], "").await.ok(),
        "7|a/b"
    );
    assert_eq!(send(&r, "GET", "/posts/7/%41/", &[], "").await.ok(), "7|A");
    assert_eq!(
        send(&r, "GET", "/posts/0/x", &[], "").await.issues(),
        issues(&[(".user_id", "too_small")])
    );
    assert_eq!(
        send(&r, "GET", "/posts/7", &[], "").await.error(),
        (404, "Not Found".into())
    );
    assert_eq!(
        send(&r, "GET", "/posts/7/8/9", &[], "").await.error(),
        (404, "Not Found".into())
    );

    assert_eq!(
        send(&r, "GET", "/c/1/2/x%20y", &[], "").await.ok(),
        "1/2/x y"
    );
    assert_eq!(
        send(&r, "GET", "/c/0/2/x", &[], "").await.issues(),
        issues(&[(".user_id", "too_small")])
    );
    let direct: Slug = validate_path_params(&[("slug", "x%20y")]).unwrap();
    assert_eq!(direct.slug, "x y");
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

/// Given a dashboard behind `vld_headers::<AuthHeaders>()` and
/// `vld_cookie::<Session>()`,
/// when clients send complete, partial, repeated and missing credentials,
/// then header names map to snake_case fields, repeats join with `", "`,
/// cookies are unquoted, decoded and coerced (from every `Cookie` header),
/// and a missing required value is a 422 `missing_field`.
#[tokio::test]
async fn headers_and_cookies_open_a_dashboard() {
    let protected = warp::path!("protected")
        .and(vld_headers::<AuthHeaders>())
        .map(|h: AuthHeaders| {
            format!(
                "auth={} rid={:?} tag={:?}",
                h.authorization, h.x_request_id, h.x_tag
            )
        });
    let dashboard = warp::path!("dashboard")
        .and(vld_cookie::<Session>())
        .map(|c: Session| {
            format!(
                "sid={} theme={:?} dark={:?}",
                c.session_id, c.theme, c.dark_mode
            )
        });
    let r = protected.or(dashboard).recover(handle_rejection);

    let resp = send(
        &r,
        "GET",
        "/protected",
        &[
            ("Authorization", b"Bearer token123"),
            ("X-Request-Id", b"abc-123"),
        ],
        "",
    )
    .await;
    assert_eq!(
        resp.ok(),
        r#"auth=Bearer token123 rid=Some("abc-123") tag=None"#
    );
    let resp = send(
        &r,
        "GET",
        "/protected",
        &[
            ("authorization", b"t"),
            ("x-tag", b"a"),
            ("x-tag", b"caf\xC3\xA9"),
        ],
        "",
    )
    .await;
    assert_eq!(resp.ok(), r#"auth=t rid=None tag=Some("a, café")"#);
    assert_eq!(
        send(&r, "GET", "/protected", &[], "").await.issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let resp = send(
        &r,
        "GET",
        "/dashboard",
        &[
            ("cookie", b"session_id=abc123; theme=dark"),
            ("cookie", b"dark_mode=true"),
        ],
        "",
    )
    .await;
    assert_eq!(
        resp.ok(),
        r#"sid=abc123 theme=Some("dark") dark=Some(true)"#
    );
    let resp = send(
        &r,
        "GET",
        "/dashboard",
        &[("cookie", b"session_id=\"abc%20def\"")],
        "",
    )
    .await;
    assert_eq!(resp.ok(), "sid=abc def theme=None dark=None");
    assert_eq!(
        send(&r, "GET", "/dashboard", &[], "").await.issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// Given an app mixing warp's own filters with vld filters, recovered by
/// `handle_rejection`,
/// when warp's filters reject (method, headers, cookies, query, length,
/// media type, size), nothing matches, or a custom rejection escapes,
/// then each becomes JSON `{"error": ...}` with warp's status (405 ranked
/// above 404), unknown rejections are 500, and a `VldRejection` built by
/// hand answers with its own status and body.
#[tokio::test]
async fn every_rejection_becomes_a_json_error() {
    #[derive(Debug)]
    struct Custom;
    impl warp::reject::Reject for Custom {}

    let only_post = warp::path!("json")
        .and(warp::post())
        .and(vld_json::<Named>())
        .map(|n: Named| n.name);
    let header = warp::path!("h")
        .and(warp::header::<u32>("x-required"))
        .map(|h: u32| h.to_string());
    let cookie = warp::path!("cookie")
        .and(warp::cookie::<String>("sid"))
        .map(|s: String| s);
    let query = warp::path!("q")
        .and(warp::query::<std::collections::HashMap<String, u8>>())
        .map(|_| "ok".to_string());
    let sized = warp::path!("sized")
        .and(warp::body::content_length_limit(4))
        .and(warp::body::json::<Value>())
        .map(|v: Value| v.to_string());
    let custom =
        warp::path!("custom").and_then(|| async { Err::<String, _>(warp::reject::custom(Custom)) });
    let mine = warp::path!("mine").and_then(|| async {
        let e = vld_web::common::VldHttpError::payload_too_large();
        Err::<String, _>(warp::reject::custom(VldRejection::from(e)))
    });
    let r = only_post
        .or(header)
        .or(cookie)
        .or(query)
        .or(sized)
        .or(custom)
        .or(mine)
        .recover(handle_rejection);

    assert_eq!(
        send(&r, "DELETE", "/json", &[], "").await.status,
        StatusCode::METHOD_NOT_ALLOWED
    );
    assert_eq!(send(&r, "GET", "/h", &[], "").await.error().0, 400);
    assert_eq!(
        send(&r, "GET", "/h", &[("x-required", b"abc")], "")
            .await
            .error()
            .0,
        400
    );
    assert_eq!(
        send(&r, "GET", "/h", &[("x-required", b"7")], "")
            .await
            .ok(),
        "7"
    );
    assert_eq!(send(&r, "GET", "/cookie", &[], "").await.error().0, 400);
    assert_eq!(
        send(&r, "GET", "/q?x=notanumber", &[], "").await.error().0,
        400
    );
    let resp = warp::test::request()
        .method("POST")
        .path("/sized")
        .reply(&r)
        .await;
    assert_eq!(resp.status(), StatusCode::LENGTH_REQUIRED);
    let resp = send(
        &r,
        "POST",
        "/sized",
        &[("content-length", b"9"), ("content-type", JSON)],
        "[1,2,3,4]",
    )
    .await;
    assert_eq!(resp.error(), (413, "Payload too large".into()));
    let resp = send(
        &r,
        "POST",
        "/sized",
        &[("content-length", b"2"), ("content-type", b"text/plain")],
        "{}",
    )
    .await;
    assert_eq!(resp.error().0, 415);
    assert_eq!(
        send(&r, "GET", "/custom", &[], "").await.error(),
        (500, "Internal Server Error".into())
    );
    assert_eq!(
        send(&r, "GET", "/mine", &[], "").await.error(),
        (413, "Payload too large".into())
    );

    let not_found = warp::path!("only").map(|| "x").recover(handle_rejection);
    assert_eq!(
        send(&not_found, "GET", "/nope", &[], "").await.error(),
        (404, "Not Found".into())
    );

    let e = VldRejection::from(vld_web::common::VldHttpError::from(
        Named::parse_value(&json!({"name": "A"})).unwrap_err(),
    ));
    assert_eq!(e.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.body()["issues"][0]["path"], ".name");
    assert_eq!(e.error().issues.len(), 1);
}

/// Given a method-filtered route next to a route that rejects with an
/// unknown custom rejection,
/// when a GET reaches both,
/// then, as in warp (any specific error beats 405), the answer is the
/// custom rejection's 500, not 405.
#[tokio::test]
#[ignore = "bug: handle_rejection answers 405 when an unknown custom rejection is combined with MethodNotAllowed; warp ranks the custom (500) higher, and warp's public API cannot tell the two apart"]
async fn custom_rejections_beat_method_not_allowed() {
    #[derive(Debug)]
    struct Custom;
    impl warp::reject::Reject for Custom {}
    let only_post = warp::post().and(warp::path!("json")).map(|| "posted");
    let custom =
        warp::path!("custom").and_then(|| async { Err::<&str, _>(warp::reject::custom(Custom)) });
    let r = only_post.or(custom).recover(handle_rejection);
    assert_eq!(
        send(&r, "GET", "/custom", &[], "").await.error(),
        (500, "Internal Server Error".into())
    );
}
