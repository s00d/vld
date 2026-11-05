use ntex::web::{self, App, HttpResponse, HttpServer};
use vld_ntex::ntex;
use vld_ntex::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64 => vld::number().int().min(13).max(150),
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
    pub struct SearchQuery {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
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
    /// `Authorization` → `authorization`, `X-Request-Id` → `x_request_id`.
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct SessionCookies {
        pub session_id: String => vld::string().min(1),
    }
}

async fn create_user(body: VldJson<CreateUser>) -> HttpResponse {
    HttpResponse::Created().body(format!(
        "Created user: {} ({}, age {})",
        body.name, body.email, body.age
    ))
}

async fn get_user(path: VldPath<UserPath>) -> HttpResponse {
    HttpResponse::Ok().body(format!("User #{}", path.id))
}

async fn search(params: VldQuery<SearchQuery>) -> HttpResponse {
    HttpResponse::Ok().body(format!("Search: q={} page={:?}", params.q, params.page))
}

/// `curl -d 'username=alice&password=secret123' localhost:8080/login`
async fn login(form: VldForm<LoginForm>) -> HttpResponse {
    HttpResponse::Ok().body(format!("Welcome, {}!", form.username))
}

/// `curl -H 'Authorization: Bearer t' -b 'session_id=abc' localhost:8080/me`
async fn me(headers: VldHeaders<AuthHeaders>, cookies: VldCookie<SessionCookies>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "auth={} request_id={:?} session={}",
        headers.authorization, headers.x_request_id, cookies.session_id
    ))
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    println!("Starting ntex server at http://127.0.0.1:8080");

    let app = || {
        App::new()
            .route("/users", web::post().to(create_user))
            .route("/users/{id}", web::get().to(get_user))
            .route("/search", web::get().to(search))
            .route("/login", web::post().to(login))
            .route("/me", web::get().to(me))
    };

    #[cfg(feature = "ntex-3")]
    let server = HttpServer::new(async move || app()).bind("127.0.0.1:8080")?;
    #[cfg(feature = "ntex-4")]
    let server =
        HttpServer::new(async move |_| app()).bind("127.0.0.1:8080", ntex::SharedCfg::default())?;

    server.run().await
}
