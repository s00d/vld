//! Regression tests for the 0.5.0 audit fixes.

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Either, Full};
use tower::{ServiceBuilder, ServiceExt};
use vld_tower::{validated, ValidateJsonLayer, ValidatedBody};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct User {
        pub name: String => vld::string().min(2),
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Reports whether the request was validated and whether the body was
/// passed through untouched (`Right`) or re-buffered (`Left`).
async fn echo(req: Request<ValidatedBody<Full<Bytes>>>) -> Result<Response<Full<Bytes>>, BoxError> {
    let user = validated::<User>(&req).map(|u| u.name);
    let kind = match req.body() {
        Either::Left(_) => "buffered",
        Either::Right(_) => "untouched",
    };
    let body = req.into_body().collect().await?.to_bytes();
    let text = format!("{user:?}|{kind}|{}", String::from_utf8_lossy(&body));
    Ok(Response::new(Full::new(Bytes::from(text))))
}

async fn call(layer: ValidateJsonLayer<User>, ct: &str, body: &str) -> (StatusCode, String) {
    let svc = ServiceBuilder::new().layer(layer).service_fn(echo);
    let req = Request::builder()
        .method("POST")
        .header("content-type", ct)
        .body(Full::new(Bytes::from(body.to_owned())))
        .unwrap();
    let resp = svc.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn mixed_case_json_content_type_is_validated() {
    let (status, _) = call(
        ValidateJsonLayer::new(),
        "Application/JSON",
        r#"{"name":"A"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn plus_json_content_type_is_validated() {
    let (status, body) = call(
        ValidateJsonLayer::new(),
        "application/vnd.api+json; charset=utf-8",
        r#"{"name":"Al"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.starts_with(r#"Some("Al")|buffered|"#), "{body}");
}

#[tokio::test]
async fn oversized_json_returns_413() {
    let (status, body) = call(
        ValidateJsonLayer::with_limit(8),
        "application/json",
        r#"{"name":"a-very-long-name"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body, r#"{"error":"Payload too large"}"#);
}

#[tokio::test]
async fn non_json_body_is_not_buffered_or_limited() {
    let (status, body) = call(
        ValidateJsonLayer::with_limit(4),
        "text/plain",
        "hello world",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "None|untouched|hello world");
}

#[tokio::test]
async fn malformed_json_returns_422_validation_shape() {
    let (status, body) = call(ValidateJsonLayer::new(), "application/json", "{").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
}

/// The layer composes with an Axum router (`Router::layer` needs an
/// `Infallible` error and an `IntoResponse` body — see
/// `examples/tower_axum.rs`).
#[tokio::test]
async fn works_as_axum_router_layer() {
    use axum::{routing::post, Extension, Router};
    async fn h(
        Extension(vld_tower::Validated(u)): Extension<vld_tower::Validated<User>>,
    ) -> String {
        u.name
    }
    let app = Router::new()
        .route("/u", post(h))
        .layer(ValidateJsonLayer::<User>::new());
    for (body, status, text) in [
        (r#"{"name":"Alice"}"#, StatusCode::OK, "Alice"),
        (r#"{"name":"A"}"#, StatusCode::UNPROCESSABLE_ENTITY, ""),
    ] {
        let req = Request::builder()
            .method("POST")
            .uri("/u")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), status, "{body}");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        if status == StatusCode::OK {
            assert_eq!(bytes, text);
        }
    }
}

/// A body whose first frame is a (non-limit) read error.
struct BrokenBody;

impl http_body::Body for BrokenBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        std::task::Poll::Ready(Some(Err(std::io::Error::other("connection reset"))))
    }
}

/// A body read error is a 422 response (like vld-rama), not a service
/// error; the value is stored as `Validated<T>`.
#[tokio::test]
async fn body_read_error_is_422_response() {
    async fn inner(
        req: Request<ValidatedBody<BrokenBody>>,
    ) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
        let _ = req.extensions().get::<vld_tower::Validated<User>>();
        Ok(Response::new(Full::new(Bytes::new())))
    }
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<User>::new())
        .service_fn(inner);
    let req = Request::builder()
        .method("POST")
        .header("content-type", "application/json")
        .body(BrokenBody)
        .unwrap();
    let resp: Result<_, std::convert::Infallible> = svc.oneshot(req).await;
    let resp = resp.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
}
