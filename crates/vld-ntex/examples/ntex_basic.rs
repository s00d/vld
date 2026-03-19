use ntex::web::{self, App, HttpResponse, HttpServer};
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

#[ntex::main]
async fn main() -> std::io::Result<()> {
    println!("Starting ntex server at http://127.0.0.1:8080");

    HttpServer::new(async || {
        App::new()
            .route("/users", web::post().to(create_user))
            .route("/users/{id}", web::get().to(get_user))
            .route("/search", web::get().to(search))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
