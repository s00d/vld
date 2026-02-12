//! Warp example showcasing vld extractors with proper response schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-warp --example warp_basic
//! ```

use serde::Serialize;
use vld_warp::prelude::*;
use warp::Filter;

// ===========================================================================
// POST /users — vld_json (JSON body)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUserRequest {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
        pub age: Option<i64> => vld::number().int().optional(),
    }
}

// ===========================================================================
// GET /users/:id — vld_param (single path param)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserIdPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct UserResponse {
        pub id: i64        => vld::number().int(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
    }
}

// ===========================================================================
// GET /posts/:user_id/:post_id — vld_path (tail params)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PostPath {
        pub user_id: i64 => vld::number().int().min(1),
        pub post_id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct PostResponse {
        pub user_id: i64 => vld::number().int(),
        pub post_id: i64 => vld::number().int(),
        pub title: String => vld::string(),
    }
}

// ===========================================================================
// GET /users/:uid/posts/:pid/comments/:cid — validate_path_params (mixed)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CommentPath {
        pub user_id: i64    => vld::number().int().min(1),
        pub post_id: i64    => vld::number().int().min(1),
        pub comment_id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CommentResponse {
        pub user_id: i64    => vld::number().int(),
        pub post_id: i64    => vld::number().int(),
        pub comment_id: i64 => vld::number().int(),
        pub text: String    => vld::string(),
    }
}

// ===========================================================================
// GET /search — vld_query (query params)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct SearchRequest {
        pub q: String      => vld::string().min(1),
        pub page: i64      => vld::number().int().min(1),
        pub limit: i64     => vld::number().int().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct SearchResponse {
        pub query: String  => vld::string(),
        pub page: i64      => vld::number().int(),
        pub limit: i64     => vld::number().int(),
        pub total: i64     => vld::number().int(),
    }
}

// ===========================================================================
// GET /health
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct HealthResponse {
        pub status: String => vld::string(),
    }
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() {
    println!("=== vld-warp example ===");
    println!();
    println!("Routes:");
    println!("  POST /users                              — JSON body (vld_json)");
    println!("  GET  /users/:id                          — single path param (vld_param)");
    println!("  GET  /posts/:user_id/:post_id            — multi tail params (vld_path)");
    println!("  GET  /users/:id/posts/:pid/comments/:cid — mixed params (validate_path_params)");
    println!("  GET  /search?q=&page=&limit=             — query params (vld_query)");
    println!("  GET  /health                             — health check");
    println!();
    println!("Example requests:");
    println!();
    println!("  # Create user:");
    println!(
        r#"  curl -s -X POST http://localhost:3030/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"Alice","email":"alice@example.com","age":30}}' | jq"#
    );
    println!();
    println!("  # Validation error (name too short):");
    println!(
        r#"  curl -s -X POST http://localhost:3030/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"A","email":"bad"}}' | jq"#
    );
    println!();
    println!("  # Get user by id (single param):");
    println!(r#"  curl -s http://localhost:3030/users/42 | jq"#);
    println!();
    println!("  # Invalid user id (< 1):");
    println!(r#"  curl -s http://localhost:3030/users/0 | jq"#);
    println!();
    println!("  # Get post (tail params):");
    println!(r#"  curl -s http://localhost:3030/posts/7/99 | jq"#);
    println!();
    println!("  # Get comment (mixed static + dynamic segments):");
    println!(r#"  curl -s http://localhost:3030/users/1/posts/2/comments/3 | jq"#);
    println!();
    println!("  # Search:");
    println!(r#"  curl -s "http://localhost:3030/search?q=hello&page=1&limit=10" | jq"#);
    println!();
    println!("  # Health check:");
    println!(r#"  curl -s http://localhost:3030/health | jq"#);
    println!();

    // POST /users — JSON body
    let create_user = warp::post()
        .and(warp::path("users"))
        .and(warp::path::end())
        .and(vld_json::<CreateUserRequest>())
        .map(|req: CreateUserRequest| {
            println!("-> POST /users  {req:?}");
            warp::reply::json(&CreateUserResponse {
                status: "created".into(),
                name: req.name,
                email: req.email,
                age: req.age,
            })
        });

    // GET /users/:id — single path param via vld_param
    let get_user = warp::get()
        .and(warp::path("users"))
        .and(vld_param::<UserIdPath>("id"))
        .and(warp::path::end())
        .map(|p: UserIdPath| {
            println!("-> GET /users/{}", p.id);
            warp::reply::json(&UserResponse {
                id: p.id,
                name: "Alice".into(),
                email: "alice@example.com".into(),
            })
        });

    // GET /posts/:user_id/:post_id — all remaining segments via vld_path
    let get_post = warp::get()
        .and(warp::path("posts"))
        .and(vld_path::<PostPath>(&["user_id", "post_id"]))
        .map(|p: PostPath| {
            println!("-> GET /posts/{}/{}", p.user_id, p.post_id);
            warp::reply::json(&PostResponse {
                user_id: p.user_id,
                post_id: p.post_id,
                title: "Hello World".into(),
            })
        });

    // GET /users/:uid/posts/:pid/comments/:cid — mixed via validate_path_params
    let get_comment = warp::get()
        .and(warp::path("users"))
        .and(warp::path::param::<String>())
        .and(warp::path("posts"))
        .and(warp::path::param::<String>())
        .and(warp::path("comments"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and_then(|uid: String, pid: String, cid: String| async move {
            validate_path_params::<CommentPath>(&[
                ("user_id", &uid),
                ("post_id", &pid),
                ("comment_id", &cid),
            ])
        })
        .map(|p: CommentPath| {
            println!(
                "-> GET /users/{}/posts/{}/comments/{}",
                p.user_id, p.post_id, p.comment_id
            );
            warp::reply::json(&CommentResponse {
                user_id: p.user_id,
                post_id: p.post_id,
                comment_id: p.comment_id,
                text: "Great post!".into(),
            })
        });

    // GET /search — query params
    let search = warp::get()
        .and(warp::path("search"))
        .and(warp::path::end())
        .and(vld_query::<SearchRequest>())
        .map(|req: SearchRequest| {
            println!("-> GET /search  {req:?}");
            warp::reply::json(&SearchResponse {
                query: req.q,
                page: req.page,
                limit: req.limit,
                total: 0,
            })
        });

    // GET /health
    let health = warp::get()
        .and(warp::path("health"))
        .and(warp::path::end())
        .map(|| {
            warp::reply::json(&HealthResponse {
                status: "ok".into(),
            })
        });

    let routes = create_user
        .or(get_user)
        .or(get_post)
        .or(get_comment)
        .or(search)
        .or(health)
        .recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
