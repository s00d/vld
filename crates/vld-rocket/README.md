# vld-rocket

[![crates.io](https://img.shields.io/crates/v/vld-rocket)](https://crates.io/crates/vld-rocket)
[![docs.rs](https://img.shields.io/docsrs/vld-rocket)](https://docs.rs/vld-rocket)

[Rocket](https://rocket.rs/) 0.5 guards that validate JSON bodies, query strings, path segments, forms, headers and cookies with [vld](https://crates.io/crates/vld) schemas, plus catchers that answer with JSON errors.

## Install

```toml
[dependencies]
vld = "0.5"
vld-rocket = "0.5"
rocket = { version = "0.5", features = ["json"] }
```

## Example

```rust
use vld_rocket::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUser>) -> String {
    format!("created {}", user.name)
}

#[rocket::launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", rocket::routes![create_user])
        .register("/", vld_catchers()) // JSON bodies for 400, 413, 415, 422
}
```

Invalid input is answered with **422** and `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`; a wrong `Content-Type` is **415**, a body over the limit is **413**. Bodies are limited by Rocket's `json` / `form` limits; while those are at Rocket's built-in defaults, 2 MiB applies.

Guards: `VldJson`, `VldForm`, `VldQuery`, `VldPath`, `VldHeaders`, `VldCookie`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/rocket>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-rocket>

MSRV: 1.85 ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
