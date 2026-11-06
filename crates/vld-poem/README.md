# vld-poem

[![crates.io](https://img.shields.io/crates/v/vld-poem)](https://crates.io/crates/vld-poem)
[![docs.rs](https://img.shields.io/docsrs/vld-poem)](https://docs.rs/vld-poem)

[Poem](https://github.com/poem-web/poem) 3 extractors that validate JSON bodies, query strings, path parameters, forms, headers and cookies with [vld](https://crates.io/crates/vld) schemas.

## Install

```toml
[dependencies]
vld = "0.5"
vld-poem = "0.5"
poem = "3"
```

## Example

```rust
use poem::{handler, post, EndpointExt, Route};
use vld_poem::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[handler]
async fn create_user(user: VldJson<CreateUser>) -> String {
    format!("created {}", user.name)
}

// `VldBodyLimit` is optional (default 2 MiB).
let app = Route::new().at("/users", post(create_user).data(VldBodyLimit(64 * 1024)));
```

Invalid input is answered with **422** and `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`; a wrong `Content-Type` is **415**, a body over the limit is **413**.

Extractors: `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/poem>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-poem>

MSRV: 1.85 ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
