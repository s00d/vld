//! Stories of a rama API using every `vld-rama` extractor and the
//! `ValidateJsonLayer`: one journey per test, each request through a real
//! `WebService` / layered service.

use std::pin::Pin;
use std::task::{Context, Poll};

use rama::extensions::ExtensionsRef;
use rama::http::body::util::BodyExt;
use rama::http::body::{Frame, StreamingBody};
use rama::http::service::web::WebService;
use rama::http::service::web::response::IntoResponse;
use rama::http::{Body, HeaderValue, Method, Request, StatusCode};
use rama::layer::AddInputExtensionLayer;
use rama::{Layer, Service};
use serde_json::{Value, json};
use vld_web::rama::prelude::*;

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

async fn send<S>(svc: &S, req: Request) -> Reply
where
    S: Service<Request, Output = rama::http::Response, Error = std::convert::Infallible>,
{
    let resp = svc.serve(req).await.unwrap();
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

fn get(uri: &str, headers: &[(&str, &[u8])]) -> Request {
    let mut req = Request::builder().uri(format!("http://example.com{uri}"));
    for (k, v) in headers {
        req = req.header(*k, HeaderValue::from_bytes(v).unwrap());
    }
    req.body(Body::empty()).unwrap()
}

fn post(uri: &str, content_type: &str, body: impl Into<Body>) -> Request {
    Request::builder()
        .method(Method::POST)
        .uri(format!("http://example.com{uri}"))
        .header("content-type", content_type)
        .body(body.into())
        .unwrap()
}

const JSON: &str = "application/json";
const FORM: &str = "application/x-www-form-urlencoded";

/// A body whose first poll fails with a non-limit error.
struct BrokenBody;

impl StreamingBody for BrokenBody {
    type Data = rama::bytes::Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(Some(Err(std::io::Error::other("connection reset"))))
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(50),
        pub age: i64 => vld::number().int().min(0),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Login {
        pub username: String => vld::string().min(3),
        pub password: String => vld::string().min(8),
    }
}

fn body_service() -> WebService<()> {
    WebService::default()
        .with_post("/users", async |user: VldJson<NewUser>| {
            format!("{}:{}", user.name, user.age)
        })
        .with_post("/login", async |VldForm(f): VldForm<Login>| {
            format!("user={}", f.username)
        })
        .with_get("/login", async |VldForm(f): VldForm<Login>| {
            format!("user={}", f.username)
        })
}

/// Given a user API with a JSON `POST /users` and a form `/login`,
/// when clients send valid bodies, schema errors, malformed JSON, wrong
/// media types, broken body streams, oversized bodies and a form on GET,
/// then valid bodies reach the handler and every failure is the documented
/// JSON error: 422 with `(path, code)` issues, 413, 415; a broken stream is a
/// parse error (not 413) and a form never reads the query string.
#[tokio::test]
async fn bodies_create_users_and_log_them_in() {
    let svc = body_service();

    assert_eq!(
        send(&svc, post("/users", JSON, r#"{"name":"Alice","age":25}"#))
            .await
            .ok(),
        "Alice:25"
    );
    let r = send(&svc, post("/users", JSON, r#"{"name":"A","age":-1}"#)).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".age", "too_small")])
    );
    let r = send(&svc, post("/users", JSON, "{")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send(&svc, post("/users", "text/plain", "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(r.json()["error"], "Unsupported Media Type");

    assert_eq!(
        send(
            &svc,
            post("/login", FORM, "username=alice&password=secret123")
        )
        .await
        .ok(),
        "user=alice"
    );
    let r = send(&svc, post("/login", FORM, "username=al&password=1")).await;
    assert_eq!(
        r.issues(),
        issues(&[(".username", "too_small"), (".password", "too_small")])
    );
    let r = send(&svc, post("/login", JSON, "{}")).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    for (uri, ct) in [("/users", JSON), ("/login", FORM)] {
        let r = send(&svc, post(uri, ct, Body::new(BrokenBody))).await;
        assert_eq!(r.issues(), issues(&[("", "parse_error")]), "{uri}");
        let big = "x".repeat(vld_web::common::DEFAULT_BODY_LIMIT + 1);
        send(&svc, post(uri, ct, big)).await.too_large();
    }

    let mut req = get("/login?username=alice&password=secret123", &[]);
    req.headers_mut()
        .insert("content-type", HeaderValue::from_static(FORM));
    let r = send(&svc, req).await;
    assert_eq!(
        r.issues(),
        issues(&[
            (".username", "missing_field"),
            (".password", "missing_field")
        ])
    );
}

/// Given the same service behind `AddInputExtensionLayer(VldBodyLimit(16))`,
/// when bodies are larger than, or exactly, 16 bytes,
/// then larger ones are 413 and one of exactly the limit passes.
#[tokio::test]
async fn the_body_limit_is_a_request_extension() {
    let svc = AddInputExtensionLayer::new(VldBodyLimit(16)).into_layer(body_service());
    send(&svc, post("/users", JSON, r#"{"name":"Alice","age":1}"#))
        .await
        .too_large();
    send(&svc, post("/login", FORM, "username=alice&password=x"))
        .await
        .too_large();
    let r = send(&svc, post("/users", JSON, r#"{"name":"Al"}   "#)).await;
    assert_eq!(r.issues(), issues(&[(".age", "missing_field")]));
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub active: Option<bool> => vld::boolean().optional(),
        pub tag: Option<Vec<String>> => vld::array(vld::string()).optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_tag: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
        pub dark_mode: Option<bool> => vld::boolean().optional(),
    }
}

/// Given a service reading the query, the path, headers and cookies,
/// when clients send valid, encoded, repeated, missing and invalid values,
/// then each source is decoded and coerced by the schema and a bad value is
/// a 422 issue at its field.
#[tokio::test]
async fn text_sources_drive_search_users_and_a_dashboard() {
    let svc = WebService::default()
        .with_get("/search", async |VldQuery(q): VldQuery<Search>| {
            format!(
                "q={} page={:?} active={:?} tag={:?}",
                q.q, q.page, q.active, q.tag
            )
        })
        .with_get("/users/{id}", async |VldPath(p): VldPath<UserPath>| {
            format!("id={}", p.id)
        })
        .with_get("/me", async |VldHeaders(h): VldHeaders<AuthHeaders>| {
            format!("auth={} tag={:?}", h.authorization, h.x_tag)
        })
        .with_get("/dash", async |VldCookie(c): VldCookie<Session>| {
            format!("sid={} dark={:?}", c.session_id, c.dark_mode)
        });

    let r = send(
        &svc,
        get(
            "/search?q=hello+w&page=3&active=true&tag=a&tag=caf%C3%A9",
            &[],
        ),
    )
    .await;
    assert_eq!(
        r.ok(),
        r#"q=hello w page=Some(3) active=Some(true) tag=Some(["a", "café"])"#
    );
    assert_eq!(
        send(&svc, get("/search?q=01234", &[])).await.ok(),
        "q=01234 page=None active=None tag=None"
    );
    assert_eq!(
        send(&svc, get("/search?page=0", &[])).await.issues(),
        issues(&[(".q", "missing_field"), (".page", "too_small")])
    );
    assert_eq!(
        send(&svc, get("/search", &[])).await.issues(),
        issues(&[(".q", "missing_field")])
    );

    assert_eq!(send(&svc, get("/users/42", &[])).await.ok(), "id=42");
    assert_eq!(
        send(&svc, get("/users/0", &[])).await.issues(),
        issues(&[(".id", "too_small")])
    );
    assert_eq!(
        send(&svc, get("/users/abc", &[])).await.issues(),
        issues(&[(".id", "invalid_type")])
    );

    let r = send(
        &svc,
        get(
            "/me",
            &[
                ("authorization", b"Bearer tok"),
                ("x-tag", b"a"),
                ("x-tag", b"caf\xC3\xA9"),
            ],
        ),
    )
    .await;
    assert_eq!(r.ok(), r#"auth=Bearer tok tag=Some("a, café")"#);
    assert_eq!(
        send(&svc, get("/me", &[])).await.issues(),
        issues(&[(".authorization", "missing_field")])
    );

    let r = send(
        &svc,
        get(
            "/dash",
            &[
                ("cookie", b"session_id=abc123; theme=dark"),
                ("cookie", b"dark_mode=true"),
            ],
        ),
    )
    .await;
    assert_eq!(r.ok(), "sid=abc123 dark=Some(true)");
    assert_eq!(
        send(&svc, get("/dash", &[("cookie", b"session_id=\"a%20b\"")]))
            .await
            .ok(),
        "sid=a b dark=None"
    );
    assert_eq!(
        send(&svc, get("/dash", &[])).await.issues(),
        issues(&[(".session_id", "missing_field")])
    );
}

/// The inner service of the layer story: echoes the validated value (via
/// `validated` and the `Validated` extension) and the body it received.
#[derive(Clone)]
struct Echo;

impl Service<Request> for Echo {
    type Output = rama::http::Response;
    type Error = std::convert::Infallible;

    async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
        let user = validated::<NewUser>(&req).map(|u| format!("{}:{}", u.name, u.age));
        let ext = req
            .extensions()
            .get_ref::<Validated<NewUser>>()
            .map(|v| format!("{v:?}|{}", v.name));
        let body = req.into_body().collect().await.unwrap().to_bytes();
        Ok(format!("{user:?}|{ext:?}|{}", String::from_utf8_lossy(&body)).into_response())
    }
}

/// Given `ValidateJsonLayer<NewUser>` (default and 16-byte limits) in front
/// of a service,
/// when requests carry valid, invalid, oversized, broken and non-JSON
/// bodies,
/// then a valid JSON body reaches the service with `Validated<NewUser>` in
/// the extensions and the original bytes, failures never reach it (422 /
/// 413 JSON), and non-JSON bodies pass untouched, whatever their size.
#[tokio::test]
async fn the_json_layer_validates_before_the_service() {
    let svc = ValidateJsonLayer::<NewUser>::new().layer(Echo);
    let body = r#"{"name":"Bob","age":30}"#;
    let r = send(&svc, post("/", JSON, body)).await;
    assert_eq!(
        r.ok(),
        format!(
            r#"Some("Bob:30")|Some("Validated(NewUser {{ name: \"Bob\", age: 30 }})|Bob")|{body}"#
        )
    );
    let r = send(
        &svc,
        post("/", "Application/JSON", r#"{"name":"B","age":-1}"#),
    )
    .await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".age", "too_small")])
    );
    let r = send(&svc, post("/", JSON, "{")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = send(&svc, post("/", JSON, Body::new(BrokenBody))).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    assert_eq!(
        send(&svc, post("/", "text/plain", "hello")).await.ok(),
        "None|None|hello"
    );
    assert_eq!(send(&svc, get("/", &[])).await.ok(), "None|None|");

    let small = ValidateJsonLayer::<NewUser>::with_limit(16).layer(Echo);
    send(&small, post("/", JSON, body)).await.too_large();
    assert_eq!(
        send(
            &small,
            post("/", "text/plain", "hello world, longer than 16")
        )
        .await
        .ok(),
        "None|None|hello world, longer than 16"
    );
    let r = send(&small, post("/", JSON, r#"{"name":"Al"}   "#)).await;
    assert_eq!(r.issues(), issues(&[(".age", "missing_field")]));
    let default = ValidateJsonLayer::<NewUser>::default().layer(Echo);
    let big = format!(
        r#"{{"name":"{}","age":1}}"#,
        "x".repeat(vld_web::common::DEFAULT_BODY_LIMIT)
    );
    send(&default, post("/", JSON, big)).await.too_large();
    assert!(
        send(&default, post("/", JSON, body))
            .await
            .ok()
            .starts_with(r#"Some("Bob:30")"#)
    );

    // Without the layer nothing was validated.
    assert_eq!(
        send(&Echo, post("/", JSON, body)).await.ok(),
        format!("None|None|{body}")
    );
}

/// Given a handler that answers with a `VldRejection` it built itself,
/// when it rejects,
/// then the response is the same JSON body the extractors send, with the
/// rejection's status.
#[tokio::test]
async fn handlers_answer_with_rejections_of_their_own() {
    let svc = WebService::default()
        .with_get("/invalid", async || {
            VldRejection::from_vld(
                NewUser::parse_value(&json!({"name": "A", "age": 1})).unwrap_err(),
            )
        })
        .with_get("/parse", async || VldRejection::parse("bad input"))
        .with_get("/large", async || VldRejection::payload_too_large());

    assert_eq!(
        send(&svc, get("/invalid", &[])).await.issues(),
        issues(&[(".name", "too_small")])
    );
    let r = send(&svc, get("/parse", &[])).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    assert_eq!(r.json()["issues"][0]["message"], "bad input");
    send(&svc, get("/large", &[])).await.too_large();

    let e = VldRejection::from(vld_web::common::VldHttpError::unsupported_media_type());
    assert_eq!(e.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(e.body()["error"], "Unsupported Media Type");
    assert_eq!(e.error().issues.len(), 1);
    assert_eq!(e.to_string(), "Unsupported Media Type");
    assert_eq!(
        VldRejection::payload_too_large().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
