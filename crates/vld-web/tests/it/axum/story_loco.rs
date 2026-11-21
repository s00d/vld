//! Story: vld extractors in a loco-rs controller. loco controllers are axum
//! handlers on a `Router<AppContext>` (loco's `Routes::new().prefix(..)
//! .add(..)` builds one), so `vld_web::axum` works there without a loco
//! dependency. `AppContext` and the error type are stand-ins with loco's
//! shapes (`State<AppContext>`, `Error::CustomError(status, ErrorDetail)`).

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use vld_web::axum::{VldJson, VldJsonRejection, VldPath};

/// Stand-in for `loco_rs::app::AppContext`.
#[derive(Clone)]
struct AppContext {
    environment: &'static str,
}

/// Stand-in for loco's `ErrorDetail` body: `{error, description, errors}`.
#[derive(serde::Serialize)]
struct ErrorDetail {
    error: String,
    description: String,
    errors: Value,
}

/// Stand-in for `loco_rs::Error::CustomError(StatusCode, ErrorDetail)`
/// (boxed: clippy's `result_large_err`).
struct Error(StatusCode, Box<ErrorDetail>);

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, axum::Json(*self.1)).into_response()
    }
}

/// A vld rejection in loco's error shape: 400 with the issues under `errors`.
impl From<VldJsonRejection> for Error {
    fn from(r: VldJsonRejection) -> Self {
        Error(
            StatusCode::BAD_REQUEST,
            Box::new(ErrorDetail {
                error: "Bad Request".into(),
                description: "invalid request".into(),
                errors: r.body()["issues"].clone(),
            }),
        )
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Params {
        pub title: String => vld::string().min(3),
        pub content: String => vld::string().max(20),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Id {
        pub id: i64 => vld::number().int().min(1),
    }
}

/// `controllers/articles.rs::add`: vld's own 422 on bad input.
async fn add(
    State(ctx): State<AppContext>,
    VldJson(p): VldJson<Params>,
) -> Result<axum::Json<Value>, Error> {
    Ok(axum::Json(
        json!({"title": p.title, "env": ctx.environment}),
    ))
}

/// `controllers/articles.rs::update`: the rejection rendered as loco's error.
async fn update(
    State(_ctx): State<AppContext>,
    path: Result<VldPath<Id>, VldJsonRejection>,
    body: Result<VldJson<Params>, VldJsonRejection>,
) -> Result<axum::Json<Value>, Error> {
    let VldPath(id) = path?;
    let VldJson(p) = body?;
    Ok(axum::Json(json!({"id": id.id, "title": p.title})))
}

/// `controllers/articles.rs::routes`: what `Routes::new().prefix("api/articles")`
/// builds, mounted by `AppRoutes` with the `AppContext` state.
fn app() -> Router {
    let routes = Router::new()
        .route("/", post(add))
        .route("/{id}", post(update));
    Router::new()
        .nest("/api/articles", routes)
        .with_state(AppContext {
            environment: "test",
        })
}

async fn post_json(uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn paths_codes(issues: &Value) -> Vec<(String, String)> {
    issues
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["path"].as_str().unwrap().into(),
                i["code"].as_str().unwrap().into(),
            )
        })
        .collect()
}

/// Given a loco-style articles controller with `AppContext` state under the
/// `/api/articles` prefix,
/// when valid and invalid articles are posted,
/// then valid ones reach the handler next to the state, `VldJson` rejects
/// with vld's 422, and a handler taking `Result<VldJson<_>, _>` renders the
/// same issues as loco's 400 `ErrorDetail`.
#[tokio::test]
async fn vld_extractors_work_in_a_loco_controller() {
    let ok = json!({"title": "Hello", "content": "short"});
    let bad = json!({"title": "Hi", "content": "x".repeat(21)});
    let expected = vec![
        (".title".to_string(), "too_small".to_string()),
        (".content".to_string(), "too_big".to_string()),
    ];

    assert_eq!(
        post_json("/api/articles", ok.clone()).await,
        (StatusCode::OK, json!({"title": "Hello", "env": "test"}))
    );
    let (status, body) = post_json("/api/articles", bad.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "Validation failed");
    assert_eq!(paths_codes(&body["issues"]), expected);

    assert_eq!(
        post_json("/api/articles/7", ok.clone()).await,
        (StatusCode::OK, json!({"id": 7, "title": "Hello"}))
    );
    let (status, body) = post_json("/api/articles/7", bad).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        (&body["error"], &body["description"]),
        (&json!("Bad Request"), &json!("invalid request"))
    );
    assert_eq!(paths_codes(&body["errors"]), expected);
    let (status, body) = post_json("/api/articles/0", ok).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        paths_codes(&body["errors"]),
        [(".id".to_string(), "too_small".to_string())]
    );
}
