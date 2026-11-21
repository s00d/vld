---
title: vld
description: Type-safe runtime validation for Rust, inspired by Zod.
navigation: false
---

Type-safe runtime validation for Rust, inspired by Zod: describe the data once, get typed values, every error with its path, and JSON Schema / OpenAPI.

```bash
cargo add vld --features derive,openapi
```

```rust [main.rs]
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<u8> => vld::u8().min(18).optional(),
    }
}

let user = User::parse(r#"{"name": "Alex", "email": "alex@example.com"}"#)?;
assert_eq!(user.age, None);
```

Start with the [introduction](/getting-started/introduction) or the [quick start](/getting-started/quick-start).
