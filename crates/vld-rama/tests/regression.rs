//! Regression tests for the 0.5.0 audit fixes.

use std::pin::Pin;
use std::task::{Context, Poll};

use rama::http::body::util::BodyExt;
use rama::http::body::{Frame, StreamingBody};
use rama::http::service::web::WebService;
use rama::http::{Body, Method, Request, StatusCode};
use rama::{Layer, Service};
use vld_rama::{validated, ValidateJsonLayer, VldForm, VldJson};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct User {
        pub name: String => vld::string().min(2),
    }
}

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

#[derive(Clone)]
struct Echo;

impl Service<Request> for Echo {
    type Output = rama::http::Response;
    type Error = std::convert::Infallible;

    async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
        let user = validated::<User>(&req).map(|u| u.name);
        let body = req.into_body().collect().await.unwrap().to_bytes();
        Ok(rama::http::Response::new(Body::from(format!(
            "{user:?}|{}",
            String::from_utf8_lossy(&body)
        ))))
    }
}

fn post(ct: &str, body: Body) -> Request {
    Request::builder()
        .method(Method::POST)
        .uri("http://example.com/")
        .header("content-type", ct)
        .body(body)
        .unwrap()
}

async fn text(resp: rama::http::Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn layer_passes_large_non_json_body_untouched() {
    let svc = ValidateJsonLayer::<User>::with_limit(4).layer(Echo);
    let resp = svc
        .serve(post("text/plain", Body::from("hello world")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(text(resp).await, "None|hello world");
}

#[tokio::test]
async fn layer_json_over_limit_is_413() {
    let svc = ValidateJsonLayer::<User>::with_limit(4).layer(Echo);
    let resp = svc
        .serve(post("application/json", Body::from(r#"{"name":"Alice"}"#)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(text(resp).await, r#"{"error":"Payload too large"}"#);
}

#[tokio::test]
async fn layer_detects_mixed_case_json() {
    let svc = ValidateJsonLayer::<User>::new().layer(Echo);
    let resp = svc
        .serve(post("Application/JSON", Body::from(r#"{"name":"A"}"#)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn layer_body_error_is_not_413() {
    let svc = ValidateJsonLayer::<User>::new().layer(Echo);
    let resp = svc
        .serve(post("application/json", Body::new(BrokenBody)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn extractor_body_error_is_not_413() {
    let svc = WebService::default()
        .with_post("/j", async |VldJson(u): VldJson<User>| u.name)
        .with_post("/f", async |VldForm(u): VldForm<User>| u.name);
    for uri in ["http://example.com/j", "http://example.com/f"] {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .body(Body::new(BrokenBody))
            .unwrap();
        let resp = svc.serve(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
    }
}

#[tokio::test]
async fn extractor_body_over_limit_is_413() {
    let svc = WebService::default()
        .with_post("/j", async |VldJson(u): VldJson<User>| u.name)
        .with_post("/f", async |VldForm(u): VldForm<User>| u.name);
    let big = "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT + 1);
    for uri in ["http://example.com/j", "http://example.com/f"] {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .body(Body::from(big.clone()))
            .unwrap();
        let resp = svc.serve(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE, "{uri}");
        assert_eq!(text(resp).await, r#"{"error":"Payload too large"}"#);
    }
}

#[tokio::test]
async fn extractor_deref() {
    let u = VldJson(User { name: "Al".into() });
    assert_eq!(u.name, "Al");
}
