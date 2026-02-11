use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use tower::{Service, ServiceBuilder, ServiceExt};
use vld_tower::{try_validated, validated, ValidateJsonLayer};

// -- Schema --

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Settings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
    }
}

// -- Dummy inner service --

async fn echo_service(
    req: Request<http_body_util::Full<Bytes>>,
) -> Result<Response<http_body_util::Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    // Return the body as-is + extensions info
    let has_validated = req.extensions().get::<CreateUser>().is_some();
    let body_bytes = req.into_body().collect().await?.to_bytes();

    let resp_body = serde_json::json!({
        "validated": has_validated,
        "body": String::from_utf8_lossy(&body_bytes).to_string(),
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(
            serde_json::to_vec(&resp_body).unwrap(),
        )))
        .unwrap())
}

fn make_json_request(body: &str) -> Request<http_body_util::Full<Bytes>> {
    Request::builder()
        .method("POST")
        .uri("/test")
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn make_text_request(body: &str) -> Request<http_body_util::Full<Bytes>> {
    Request::builder()
        .method("POST")
        .uri("/test")
        .header("content-type", "text/plain")
        .body(http_body_util::Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

// -- Tests --

#[tokio::test]
async fn valid_json_passes_through() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"name": "Alice", "email": "alice@example.com"}"#);
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["validated"], true);
}

#[tokio::test]
async fn invalid_json_returns_422() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"name": "A", "email": "bad"}"#);
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
    assert!(json["issues"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn malformed_json_returns_400() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_json_request("not json at all");
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Invalid JSON");
}

#[tokio::test]
async fn non_json_content_type_passes_through() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_text_request("hello world");
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn missing_fields_returns_422() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"name": "Bob"}"#);
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["path"].as_str().unwrap().contains("email")));
}

#[tokio::test]
async fn empty_body_returns_400() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = Request::builder()
        .method("POST")
        .uri("/test")
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();

    let resp = svc.oneshot(req).await.unwrap();
    // Empty body is invalid JSON
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn validated_helper() {
    // Test the `validated` and `try_validated` helpers
    async fn handler_with_extract(
        req: Request<http_body_util::Full<Bytes>>,
    ) -> Result<Response<http_body_util::Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let user: CreateUser = validated(&req);
        let resp_body = serde_json::json!({
            "name": user.name,
            "email": user.email,
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(http_body_util::Full::new(Bytes::from(
                serde_json::to_vec(&resp_body).unwrap(),
            )))
            .unwrap())
    }

    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(handler_with_extract);

    let req = make_json_request(r#"{"name": "Alice", "email": "alice@test.com"}"#);
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "Alice");
    assert_eq!(json["email"], "alice@test.com");
}

#[tokio::test]
async fn try_validated_returns_none_without_middleware() {
    async fn handler(
        req: Request<http_body_util::Full<Bytes>>,
    ) -> Result<Response<http_body_util::Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let user: Option<CreateUser> = try_validated(&req);
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(http_body_util::Full::new(Bytes::from(
                if user.is_some() { "found" } else { "none" }.to_string(),
            )))
            .unwrap())
    }

    // No middleware applied
    let mut svc = tower::service_fn(handler);
    let req = make_json_request(r#"{"name": "Alice", "email": "a@b.com"}"#);
    let resp = svc.call(req).await.unwrap();

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"none");
}

#[tokio::test]
async fn different_schema_type() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<Settings>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"host": "localhost", "port": 8080}"#);
    let resp = svc.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn different_schema_invalid() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<Settings>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"host": "", "port": 99999}"#);
    let resp = svc.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn error_response_has_content_type() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(echo_service);

    let req = make_json_request(r#"{"name": "A", "email": "bad"}"#);
    let resp = svc.oneshot(req).await.unwrap();

    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn layer_is_clone() {
    let layer = ValidateJsonLayer::<CreateUser>::new();
    let _clone = layer.clone();
}

#[tokio::test]
async fn layer_default() {
    let _layer = ValidateJsonLayer::<CreateUser>::default();
}
