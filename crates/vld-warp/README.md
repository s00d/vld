[![Crates.io](https://img.shields.io/crates/v/vld-warp?style=for-the-badge)](https://crates.io/crates/vld-warp)
[![docs.rs](https://img.shields.io/docsrs/vld-warp?style=for-the-badge)](https://docs.rs/vld-warp)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-warp

[Warp](https://docs.rs/warp) integration for the **vld** validation library.

## Features

| Filter / Function | Source | Description |
|-------------------|--------|-------------|
| `vld_json::<T>()` | JSON body | Validates JSON request body |
| `vld_query::<T>()` | Query string | Validates URL query parameters |
| `vld_form::<T>()` | Form body | Validates URL-encoded form body |
| `vld_param::<T>(name)` | Single path segment | Validates one URL path segment |
| `vld_path::<T>(names)` | Path tail | Validates all remaining path segments |
| `validate_path_params::<T>(pairs)` | Pre-extracted params | Validates mixed static/dynamic path segments |
| `vld_headers::<T>()` | HTTP headers | Validates request headers |
| `vld_cookie::<T>()` | Cookie header | Validates cookie values |
| `handle_rejection` | — | Converts vld rejections into JSON responses |

Validation failures are returned as `422 Unprocessable Entity` with a JSON error body.

## Installation

```toml
[dependencies]
vld-warp = "0.1"
vld = "0.1"
warp = "0.3"
serde_json = "1"
```

## Quick Start

```rust
use vld_warp::prelude::*;
use warp::Filter;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[tokio::main]
async fn main() {
    let route = warp::post()
        .and(warp::path("users"))
        .and(warp::path::end())
        .and(vld_json::<CreateUser>())
        .map(|u: CreateUser| {
            warp::reply::json(&serde_json::json!({"name": u.name}))
        })
        .recover(handle_rejection);

    warp::serve(route).run(([0, 0, 0, 0], 3030)).await;
}
```

## Path Parameters

Warp doesn't have a built-in named path parameter extractor. `vld-warp`
provides three approaches to fill this gap.

### `vld_param` — single segment

Extracts **one** path segment and validates it. The `name` argument
becomes the JSON key so it matches your schema's field name.

```rust,ignore
vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

// GET /users/<id>
let route = warp::path("users")
    .and(vld_param::<UserId>("id"))
    .and(warp::path::end())
    .map(|p: UserId| warp::reply::json(&serde_json::json!({"id": p.id})));
```

### `vld_path` — all remaining segments (tail)

Consumes **all remaining** path segments via `warp::path::tail()` and
maps them 1-to-1 to the provided names. The segment count must match
the name count exactly (otherwise → 404).

Best for routes where **all** remaining segments are dynamic.

```rust,ignore
vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PostPath {
        pub user_id: i64 => vld::number().int().min(1),
        pub post_id: i64 => vld::number().int().min(1),
    }
}

// GET /posts/<user_id>/<post_id>
let route = warp::path("posts")
    .and(vld_path::<PostPath>(&["user_id", "post_id"]))
    .map(|p: PostPath| {
        warp::reply::json(&serde_json::json!({
            "user_id": p.user_id,
            "post_id": p.post_id
        }))
    });
```

### `validate_path_params` — mixed static / dynamic

For complex routes with interleaved static and dynamic segments, extract
each `String` segment with `warp::path::param::<String>()` yourself,
then call `validate_path_params` in `and_then` to validate them all at
once.

```rust,ignore
vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CommentPath {
        pub user_id: i64    => vld::number().int().min(1),
        pub post_id: i64    => vld::number().int().min(1),
        pub comment_id: i64 => vld::number().int().min(1),
    }
}

// GET /users/<uid>/posts/<pid>/comments/<cid>
let route = warp::path("users")
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
    });
```

## Recovery Handler

Always add `.recover(handle_rejection)` to convert vld rejections into structured JSON:

```rust,ignore
let routes = create.or(search).recover(handle_rejection);
```

Error response format:

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": ".name", "message": "String must be at least 2 characters" }
  ]
}
```

## Running Examples

```bash
cargo run -p vld-warp --example warp_basic
```

### Example Requests

```bash
# Create user (valid)
curl -X POST http://localhost:3030/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","age":30}'

# Create user (invalid — triggers 422)
curl -X POST http://localhost:3030/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"A","email":"bad","age":-1}'

# Get user by id (vld_param)
curl http://localhost:3030/users/42

# Invalid user id (vld_param — triggers 422)
curl http://localhost:3030/users/0

# Get post (vld_path — tail params)
curl http://localhost:3030/posts/7/99

# Get comment (validate_path_params — mixed segments)
curl http://localhost:3030/users/1/posts/2/comments/3

# Search (query params)
curl "http://localhost:3030/search?q=hello&page=1&limit=10"

# Health check
curl http://localhost:3030/health
```

## License

MIT
