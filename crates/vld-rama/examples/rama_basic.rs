//! Rama + vld extractors example.
//!
//! Run:
//! ```sh
//! cargo run -p vld-rama --example rama_basic
//! ```

use rama::http::service::web::WebService;
use rama::http::service::web::response::Json;
use rama::http::{Body, Method, Request, StatusCode};
use rama::{Layer, Service};
use serde::Serialize;
use vld_rama::{
    ValidateJsonLayer, VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery, validated,
};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String => vld::string(),
        pub email: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
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
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
    }
}

async fn get_user(VldPath(p): VldPath<UserPath>) -> String {
    format!("user #{}", p.id)
}

async fn login(VldForm(f): VldForm<LoginForm>) -> String {
    format!("Welcome, {}!", f.username)
}

async fn me(VldCookie(c): VldCookie<Session>) -> String {
    format!("session={}", c.session_id)
}

/// Downstream of `ValidateJsonLayer` (proxy / gateway style).
#[derive(Clone)]
struct Backend;

impl Service<Request> for Backend {
    type Output = rama::http::Response;
    type Error = std::convert::Infallible;

    async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
        let name = validated::<CreateUser>(&req).map_or("-".into(), |u| u.name);
        Ok(rama::http::Response::new(Body::from(name)))
    }
}

fn request(method: Method, uri: &str, header: Option<(&str, &str)>, body: &'static str) -> Request {
    let mut b = Request::builder()
        .method(method)
        .uri(format!("http://example.com{uri}"));
    if let Some((k, v)) = header {
        b = b.header(k, v);
    }
    b.body(Body::from(body)).unwrap()
}

async fn create_user(VldJson(req): VldJson<CreateUser>) -> Json<CreateUserResponse> {
    Json(CreateUserResponse {
        status: "created".into(),
        name: req.name,
        email: req.email,
    })
}

async fn search(VldQuery(q): VldQuery<Search>) -> String {
    format!("q={} page={}", q.q, q.page.unwrap_or(1))
}

async fn authed(
    VldHeaders(h): VldHeaders<AuthHeaders>,
    VldJson(user): VldJson<CreateUser>,
) -> String {
    format!("auth={} name={}", h.authorization, user.name)
}

#[tokio::main]
async fn main() {
    let svc = WebService::default()
        .with_post("/users", create_user)
        .with_get("/search", search)
        .with_post("/authed", authed)
        .with_get("/users/{id}", get_user)
        .with_post("/login", login)
        .with_get("/me", me);

    // --- create user ---
    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/users")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"name":"Alice","email":"alice@example.com"}"#,
        ))
        .unwrap();
    let resp = svc.serve(req).await.unwrap();
    println!("POST /users => {}", resp.status());
    assert_eq!(resp.status(), StatusCode::OK);

    // --- search ---
    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/search?q=hello&page=2")
        .body(Body::empty())
        .unwrap();
    let resp = svc.serve(req).await.unwrap();
    println!("GET /search => {}", resp.status());
    assert_eq!(resp.status(), StatusCode::OK);

    // --- validation error ---
    let req = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/users")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"A","email":"bad"}"#))
        .unwrap();
    let resp = svc.serve(req).await.unwrap();
    println!("POST /users (invalid) => {}", resp.status());
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // --- path / form / cookie ---
    for req in [
        request(Method::GET, "/users/42", None, ""),
        request(
            Method::POST,
            "/login",
            Some(("content-type", "application/x-www-form-urlencoded")),
            "username=alice",
        ),
        request(Method::GET, "/me", Some(("cookie", "session_id=abc")), ""),
    ] {
        let line = format!("{} {}", req.method(), req.uri());
        let resp = svc.serve(req).await.unwrap();
        println!("{line} => {}", resp.status());
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // --- layer ---
    let gateway = ValidateJsonLayer::<CreateUser>::new().layer(Backend);
    let req = request(
        Method::POST,
        "/",
        Some(("content-type", "application/json")),
        r#"{"name":"Bob","email":"bob@example.com"}"#,
    );
    let resp = gateway.serve(req).await.unwrap();
    println!("layer => {}", resp.status());
    assert_eq!(resp.status(), StatusCode::OK);

    println!("ok");
}
