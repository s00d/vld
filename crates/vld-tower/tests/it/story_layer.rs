//! Stories of a tower stack validating JSON bodies with
//! `ValidateJsonLayer`: one journey per test, each request through a real
//! layered service.

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Either, Full};
use serde_json::Value;
use std::task::{Context, Poll};
use tower::{Service, ServiceBuilder, ServiceExt};
use vld_tower::prelude::*;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct NewUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Settings {
        pub host: String => vld::string().min(1),
        pub port: i64 => vld::number().int().min(1).max(65535),
    }
}

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    text: String,
}

impl Reply {
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
        let body: Value = serde_json::from_str(&self.text).unwrap();
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

/// Echoes what the inner service sees: the validated value (via
/// `validated` and the `Validated` extension), whether the body was
/// re-buffered (`Left`) or passed untouched (`Right`), and the body.
async fn echo<B>(req: Request<ValidatedBody<B>>) -> Result<Response<Full<Bytes>>, BoxError>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let user = validated::<NewUser>(&req).map(|u| u.name);
    let ext = req
        .extensions()
        .get::<Validated<NewUser>>()
        .map(|v| v.email.clone());
    let kind = match req.body() {
        Either::Left(_) => "buffered",
        Either::Right(_) => "untouched",
    };
    let body = req.into_body().collect().await?.to_bytes();
    let text = format!("{user:?}|{ext:?}|{kind}|{}", String::from_utf8_lossy(&body));
    Ok(Response::new(Full::new(Bytes::from(text))))
}

async fn call<S, B>(svc: S, req: Request<B>) -> Reply
where
    S: Service<Request<B>, Response = Response<ValidatedBody<Full<Bytes>>>>,
    S::Error: std::fmt::Debug,
{
    let resp = svc.oneshot(req).await.unwrap();
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

fn req(content_type: &str, body: &str) -> Request<Full<Bytes>> {
    Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", content_type)
        .body(Full::new(Bytes::from(body.to_owned())))
        .unwrap()
}

/// A body whose first frame is a (non-limit) read error.
struct BrokenBody;

impl http_body::Body for BrokenBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        Poll::Ready(Some(Err(std::io::Error::other("connection reset"))))
    }
}

const JSON: &str = "application/json";

/// Given `ValidateJsonLayer<NewUser>` in front of a service,
/// when requests carry valid, invalid, incomplete, malformed, empty, broken
/// and non-JSON bodies, in every JSON media-type spelling,
/// then a valid body reaches the service re-buffered with the value in
/// `Validated<NewUser>`; every failure is a JSON 422 with `(path, code)`
/// issues and never reaches the service; non-JSON bodies pass untouched.
#[tokio::test]
async fn the_json_layer_validates_before_the_service() {
    let layer = ValidateJsonLayer::<NewUser>::new();
    let svc = || ServiceBuilder::new().layer(layer.clone()).service_fn(echo);

    let body = r#"{"name":"Alice","email":"alice@example.com"}"#;
    let r = call(svc(), req(JSON, body)).await;
    assert_eq!(
        r.ok(),
        format!(r#"Some("Alice")|Some("alice@example.com")|buffered|{body}"#)
    );
    let r = call(
        svc(),
        req(
            "application/vnd.api+json; charset=utf-8",
            r#"{"name":"Al","email":"a@b.co"}"#,
        ),
    )
    .await;
    assert!(r.ok().starts_with(r#"Some("Al")|Some("a@b.co")|buffered|"#));

    let r = call(svc(), req(JSON, r#"{"name":"A","email":"bad"}"#)).await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    let r = call(svc(), req("Application/JSON", r#"{"name":"Bob"}"#)).await;
    assert_eq!(r.issues(), issues(&[(".email", "missing_field")]));
    let r = call(svc(), req(JSON, "not json at all")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let r = call(svc(), req(JSON, "")).await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));
    let broken = Request::builder()
        .method("POST")
        .header("content-type", JSON)
        .body(BrokenBody)
        .unwrap();
    let r = call(
        ServiceBuilder::new().layer(layer.clone()).service_fn(echo),
        broken,
    )
    .await;
    assert_eq!(r.issues(), issues(&[("", "parse_error")]));

    assert_eq!(
        call(svc(), req("text/plain", "hello world")).await.ok(),
        "None|None|untouched|hello world"
    );
    let no_type = Request::builder()
        .method("POST")
        .body(Full::new(Bytes::from("x")))
        .unwrap();
    assert_eq!(call(svc(), no_type).await.ok(), "None|None|untouched|x");

    // `Default` builds the same layer; another schema type works the same.
    let r = call(
        ServiceBuilder::new()
            .layer(ValidateJsonLayer::<NewUser>::default())
            .service_fn(echo),
        req(JSON, "{}"),
    )
    .await;
    assert_eq!(
        r.issues(),
        issues(&[(".name", "missing_field"), (".email", "missing_field")])
    );
    let settings = || {
        ServiceBuilder::new()
            .layer(ValidateJsonLayer::<Settings>::new())
            .service_fn(echo)
    };
    assert_eq!(
        call(settings(), req(JSON, r#"{"host":"localhost","port":8080}"#))
            .await
            .ok(),
        r#"None|None|buffered|{"host":"localhost","port":8080}"#
    );
    let r = call(settings(), req(JSON, r#"{"host":"","port":99999}"#)).await;
    assert_eq!(
        r.issues(),
        issues(&[(".host", "too_small"), (".port", "too_big")])
    );
}

/// Given layers with an 8-byte limit and the 2 MiB default,
/// when JSON bodies exceed them (and a large non-JSON body arrives),
/// then the JSON ones are 413 with the JSON body, a JSON body of exactly
/// the limit is read, and the non-JSON body is neither buffered nor limited.
#[tokio::test]
async fn bodies_above_the_limit_are_413() {
    let small = || {
        ServiceBuilder::new()
            .layer(ValidateJsonLayer::<NewUser>::with_limit(8))
            .service_fn(echo)
    };
    call(small(), req(JSON, r#"{"name":"a-very-long-name"}"#))
        .await
        .too_large();
    assert_eq!(call(small(), req(JSON, "{}      ")).await.issues().len(), 2);
    assert_eq!(
        call(small(), req("text/plain", "hello world")).await.ok(),
        "None|None|untouched|hello world"
    );

    let default = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<NewUser>::new())
        .service_fn(echo);
    let big = format!(
        r#"{{"name":"{}"}}"#,
        "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT)
    );
    call(default, req(JSON, &big)).await.too_large();
}

/// An inner service that is never ready.
#[derive(Clone)]
struct Down;

impl Service<Request<ValidatedBody<Full<Bytes>>>> for Down {
    type Response = Response<Full<Bytes>>;
    type Error = BoxError;
    type Future = std::future::Ready<Result<Self::Response, BoxError>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), BoxError>> {
        Poll::Ready(Err("down".into()))
    }

    fn call(&mut self, _: Request<ValidatedBody<Full<Bytes>>>) -> Self::Future {
        std::future::ready(Ok(Response::new(Full::new(Bytes::from("called")))))
    }
}

/// Given the layer in front of an inner service that reports an error from
/// `poll_ready`, and a plain service without the layer,
/// when a request is sent,
/// then the layer surfaces the inner readiness error, and without the layer
/// `validated` finds nothing.
#[tokio::test]
async fn the_layer_forwards_readiness_and_needs_to_be_applied() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<NewUser>::new())
        .service(Down);
    let err = svc.oneshot(req(JSON, "{}")).await.unwrap_err();
    assert_eq!(err.to_string(), "down");

    let plain = tower::service_fn(|r: Request<Full<Bytes>>| async move {
        Ok::<_, BoxError>(validated::<NewUser>(&r).is_some())
    });
    assert!(
        !plain
            .oneshot(req(JSON, r#"{"name":"Alice","email":"a@b.co"}"#))
            .await
            .unwrap()
    );
}

/// Given the layer on an axum `Router` (the handler reads
/// `Extension<Validated<NewUser>>`),
/// when valid and invalid bodies are posted,
/// then the handler gets the value and invalid bodies are 422 before it.
#[tokio::test]
async fn the_layer_composes_with_an_axum_router() {
    use axum::{Extension, Router, routing::post};
    async fn h(Extension(Validated(u)): Extension<Validated<NewUser>>) -> String {
        u.name
    }
    let app = Router::new()
        .route("/u", post(h))
        .layer(ValidateJsonLayer::<NewUser>::new());
    for (body, status, text) in [
        (
            r#"{"name":"Alice","email":"a@b.co"}"#,
            StatusCode::OK,
            "Alice",
        ),
        (
            r#"{"name":"A","email":"a@b.co"}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
            "",
        ),
    ] {
        let req = Request::builder()
            .method("POST")
            .uri("/u")
            .header("content-type", JSON)
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), status, "{body}");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        if status == StatusCode::OK {
            assert_eq!(bytes, text);
        } else {
            let got: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(got["issues"][0]["path"], ".name");
            assert_eq!(got["issues"][0]["code"], "too_small");
        }
    }
}
