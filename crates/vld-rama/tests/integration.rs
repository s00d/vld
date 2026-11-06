use rama::extensions::ExtensionsRef;
use rama::http::body::util::BodyExt;
use rama::http::service::web::WebService;
use rama::http::{Body, Method, Request, StatusCode};
use rama::{Layer, Service};
use vld_rama::{
    ValidateJsonLayer, Validated, VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery,
    validated,
};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub age: i64 => vld::number().int().min(0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct SearchQuery {
        pub q: String => vld::string().min(1),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
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
    pub struct LoginForm {
        pub username: String => vld::string().min(3),
        pub password: String => vld::string().min(8),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct SessionCookies {
        pub session_id: String => vld::string().min(1),
    }
}

async fn body_text(resp: rama::http::Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn valid_json_request() {
    let svc = WebService::default().with_post("/test", async |VldJson(user): VldJson<TestUser>| {
        format!("{}:{}", user.name, user.age)
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/test")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Alice","age":25}"#))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "Alice:25");
}

#[tokio::test]
async fn invalid_json_returns_422() {
    let svc = WebService::default()
        .with_post("/test", async |VldJson(_user): VldJson<TestUser>| {
            StatusCode::OK
        });

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/test")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"A","age":-1}"#))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let text = body_text(resp).await;
    assert!(text.contains("Validation failed"));
    assert!(text.contains("issues"));
}

#[tokio::test]
async fn valid_query() {
    let svc = WebService::default()
        .with_get("/search", async |VldQuery(q): VldQuery<SearchQuery>| {
            format!("{}:{}", q.q, q.page.unwrap_or(1))
        });

    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/search?q=hello&page=3")
        .body(Body::empty())
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "hello:3");
}

#[tokio::test]
async fn valid_path() {
    let svc = WebService::default()
        .with_get("/users/{id}", async |VldPath(p): VldPath<UserPath>| {
            format!("id={}", p.id)
        });

    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/users/42")
        .body(Body::empty())
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "id=42");
}

#[tokio::test]
async fn valid_form() {
    let svc = WebService::default().with_post("/login", async |VldForm(f): VldForm<LoginForm>| {
        format!("user={}", f.username)
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/login")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=secret123"))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "user=alice");
}

#[tokio::test]
async fn valid_headers() {
    let svc = WebService::default()
        .with_get("/me", async |VldHeaders(h): VldHeaders<AuthHeaders>| {
            format!("auth={}", h.authorization)
        });

    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/me")
        .header("authorization", "Bearer tok")
        .body(Body::empty())
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "auth=Bearer tok");
}

#[tokio::test]
async fn valid_cookie() {
    let svc = WebService::default()
        .with_get("/dash", async |VldCookie(c): VldCookie<SessionCookies>| {
            format!("sid={}", c.session_id)
        });

    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/dash")
        .header("cookie", "session_id=abc123; theme=dark")
        .body(Body::empty())
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "sid=abc123");
}

#[tokio::test]
async fn validate_json_layer_inserts_extension() {
    #[derive(Clone)]
    struct Inner;

    impl Service<Request> for Inner {
        type Output = rama::http::Response;
        type Error = std::convert::Infallible;

        async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
            let user = validated::<TestUser>(&req).expect("validated");
            assert!(validated::<TestUser>(&req).is_some());
            assert!(req.extensions().get_ref::<Validated<TestUser>>().is_some());
            Ok(rama::http::Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(format!("{}:{}", user.name, user.age)))
                .unwrap())
        }
    }

    let svc = ValidateJsonLayer::<TestUser>::new().layer(Inner);

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Bob","age":30}"#))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "Bob:30");
}

#[tokio::test]
async fn validate_json_layer_rejects_invalid() {
    #[derive(Clone)]
    struct Inner;

    impl Service<Request> for Inner {
        type Output = rama::http::Response;
        type Error = std::convert::Infallible;

        async fn serve(&self, _req: Request) -> Result<Self::Output, Self::Error> {
            use rama::http::service::web::response::IntoResponse;
            Ok(StatusCode::OK.into_response())
        }
    }

    let svc = ValidateJsonLayer::<TestUser>::new().layer(Inner);

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"B","age":-1}"#))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn validate_json_layer_passthrough_non_json() {
    #[derive(Clone)]
    struct Inner;

    impl Service<Request> for Inner {
        type Output = rama::http::Response;
        type Error = std::convert::Infallible;

        async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
            use rama::http::service::web::response::IntoResponse;
            assert!(validated::<TestUser>(&req).is_none());
            Ok(StatusCode::OK.into_response())
        }
    }

    let svc = ValidateJsonLayer::<TestUser>::new().layer(Inner);

    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/")
        .header("content-type", "text/plain")
        .body(Body::from("hello"))
        .unwrap();

    let resp = svc.serve(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
