---
title: vld
description: Type-safe runtime validation for Rust, inspired by Zod.
navigation: false
---

```toml [Cargo.toml]
[dependencies]
vld = { version = "0.5", features = ["derive", "openapi"] }
```

```rust [main.rs]
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(18).optional(),
    }
}

let user = User::parse(r#"{"name": "Alex", "email": "alex@example.com"}"#)?;
assert_eq!(user.age, None);
```
