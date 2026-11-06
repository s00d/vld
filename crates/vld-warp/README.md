# vld-warp

[![crates.io](https://img.shields.io/crates/v/vld-warp.svg)](https://crates.io/crates/vld-warp)
[![docs.rs](https://img.shields.io/docsrs/vld-warp)](https://docs.rs/vld-warp)

[Warp](https://docs.rs/warp) filters (`vld_json`, `vld_query`, `vld_form`, `vld_param`, `vld_path`, `vld_headers`, `vld_cookie`) and a JSON `handle_rejection` for the [vld](https://crates.io/crates/vld) validation library. Works with warp 0.3 and 0.4.

## Install

```toml
[dependencies]
vld = "0.5"
vld-warp = "0.5"
warp = { version = "0.4", features = ["server"] }
```

## Example

```rust
use vld_warp::prelude::*;
use warp::Filter;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

let create = warp::post()
    .and(warp::path("users"))
    .and(vld_json::<CreateUser>())
    .map(|u: CreateUser| warp::reply::json(&serde_json::json!({ "name": u.name })));

warp::serve(create.recover(handle_rejection))
    .run(([127, 0, 0, 1], 3030))
    .await;
```

With `handle_rejection`, invalid input is a 422 JSON response, a body over 2 MiB a 413, a missing JSON `Content-Type` a 415.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/warp>
- API: <https://docs.rs/vld-warp>

MSRV: Rust 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
